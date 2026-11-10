import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {ensureActorVisibleInScrollView} from 'resource:///org/gnome/shell/misc/animationUtils.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import * as Panel from './panel.js';
import {ProviderService} from './provider.js';

const RECENT_MULLVAD_LIMIT = 5;
const PHRASE_INTERVAL_MS = 2800;
const SCROLL_WORK_AREA_SHARE = 0.6;
const SCROLL_MIN_HEIGHT = 200;

// A resolved row is carried on the menu item built from it, and the shell's own
// _delegate convention is how an actor points back at that item. Neither is in
// the St or PopupMenu type definitions, so the shape is named once here.
type RowItem = PopupMenu.PopupBaseMenuItem & {
    _rowId?: string;
    _row?: Panel.PanelRow;
    _sublabel?: St.Label;
    readonly label?: St.Label;
    readonly icon?: St.Icon;
    readonly menu?: PopupMenu.PopupSubMenu;
};

type RowSection = PopupMenu.PopupMenuSection;

// _getMenuItems() is typed for the shell's own uses; every item this panel puts
// in a section is one of ours.
function menuItems(owner: {_getMenuItems(): unknown[]}): RowItem[] {
    return owner._getMenuItems() as RowItem[];
}

type Delegated = Clutter.Actor & {_delegate?: {_row?: Panel.PanelRow}; _row?: Panel.PanelRow};

// The three helpers below probe for the spelling the running shell has. A
// single release's type definitions can only describe one of them, so the
// property tests are the point and the cast is what lets them be written.
type Loose = Record<string, any>;

// St.BoxLayout.vertical was replaced by the Clutter orientation property in
// GNOME 48; both spellings have to work across the supported shell versions.
function box(vertical: boolean, props: Loose = {}): St.BoxLayout {
    const b = new St.BoxLayout(props);
    const loose = b as unknown as Loose;
    if ('orientation' in loose)
        loose.orientation = vertical ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL;
    else
        loose.vertical = vertical;
    return b;
}

// St.ScrollView took its content through add_actor until GNOME 46 turned it
// into a property, and only gained an adjustment of its own in that release.
// A GNOME 45 ScrollView is an St.Bin and so has a `child` too, but setting it
// skips the hook that wires the scrollbars: the section drew 1px tall there.
// `vadjustment` is what tells the two apart, as it does in verticalAdjustment.
function scrollView(child: Clutter.Actor, props: Loose = {}): St.ScrollView {
    const view = new St.ScrollView({
        hscrollbar_policy: St.PolicyType.NEVER,
        vscrollbar_policy: St.PolicyType.AUTOMATIC,
        ...props,
    });
    const loose = view as unknown as Loose;
    if ('vadjustment' in loose)
        loose.child = child;
    else
        loose.add_actor(child);
    return view;
}

function verticalAdjustment(view: St.ScrollView): St.Adjustment {
    const loose = view as unknown as Loose;
    return 'vadjustment' in loose ? loose.vadjustment : loose.vscroll.adjustment;
}

type PathStep = ['M' | 'L', number, number] | ['C', number, number, number, number, number, number];

// NetBird's mark: the brand SVG's paths, unmodified but made absolute, since
// cairo takes no path data. Its viewBox is "0.015 36.7 512 405.4".
const NETBIRD_VIEW = {x: 0.015, y: 36.7, width: 512, height: 405.4};
const NETBIRD_PATHS: PathStep[][] = [
    [
        ['M', 363.915, 69.9],
        ['C', 302.115, 75.6, 271.415, 111.2, 259.815, 129.2],
        ['L', 254.615, 138.3],
        ['C', 254.215, 139.1, 254.015, 139.6, 254.015, 139.6],
        ['L', 253.915, 139.5],
        ['L', 79.115, 442.1],
        ['L', 297.115, 442.1],
        ['L', 512.015, 69.9],
    ],
    [
        ['M', 297.115, 442.1],
        ['L', 0.015, 126.9],
        ['C', 0.015, 126.9, 336.015, 36.7, 368.715, 318.3],
    ],
    [
        ['M', 253.115, 140.8],
        ['L', 161.915, 298.7],
        ['L', 297.115, 442.1],
        ['L', 368.715, 318.1],
        ['C', 357.415, 221.2, 310.215, 168.4, 253.115, 140.8],
    ],
];

// ZeroTier's mark (Simple Icons, CC0-1.0) in its 24x24 viewBox: a rounded
// square with the T and its ring cut out. Cairo takes no path data, so it is
// drawn from its shapes: the square, the cut-outs erased, the halves of the
// disc inside the ring filled back.
const ZEROTIER_VIEW = 24;
function drawZeroTier(cr: Loose, left: number, top: number, size: number): void {
    const k = size / ZEROTIER_VIEW;
    const disc = (x: number, y: number, radius: number) => {
        cr.newSubPath();
        cr.arc(left + x * k, top + y * k, radius * k, 0, 2 * Math.PI);
    };
    const box = (x0: number, y0: number, x1: number, y1: number) =>
        cr.rectangle(left + x0 * k, top + y0 * k, (x1 - x0) * k, (y1 - y0) * k);

    cr.pushGroup();
    const corner = 4;
    cr.newSubPath();
    cr.arc(left + (ZEROTIER_VIEW - corner) * k, top + corner * k, corner * k, -Math.PI / 2, 0);
    cr.arc(left + (ZEROTIER_VIEW - corner) * k, top + (ZEROTIER_VIEW - corner) * k, corner * k, 0, Math.PI / 2);
    cr.arc(left + corner * k, top + (ZEROTIER_VIEW - corner) * k, corner * k, Math.PI / 2, Math.PI);
    cr.arc(left + corner * k, top + corner * k, corner * k, Math.PI, 1.5 * Math.PI);
    cr.closePath();
    cr.fill();

    cr.setOperator(0);
    box(3.338, 2.834, 20.664, 3.971);
    disc(3.338, 3.4025, 0.5685);
    disc(20.664, 3.4025, 0.5685);
    cr.fill();
    box(11.432, 3.4, 12.568, 20.597);
    disc(12, 20.597, 0.568);
    cr.fill();
    disc(12, 12, 6.057);
    cr.fill();

    cr.setOperator(2);
    for (const [x0, x1] of [[0, 11.432], [12.568, ZEROTIER_VIEW]]) {
        cr.save();
        box(x0, 0, x1, ZEROTIER_VIEW);
        cr.clip();
        disc(12, 12, 4.889);
        cr.fill();
        cr.restore();
    }
    cr.popGroupToSource();
    cr.paint();
}

// Native rendering of the provider's mark: Tailscale's 3x3 dot grid with the
// inactive dots faded, NetBird's or ZeroTier's, plus the disconnected slash
// and the needs-login badge.
const TailGaugeIcon = GObject.registerClass(
class TailGaugeIcon extends St.DrawingArea {
    declare _crossed: boolean;
    declare _warning: boolean;
    declare _providerId: string;

    override _init(params: Loose = {}): void {
        super._init({
            style_class: 'system-status-icon tailgauge-icon',
            ...params,
        });
        this._crossed = false;
        this._warning = false;
        this._providerId = '';
        this.connect('repaint', () => this._repaint());
    }

    setState(crossed: boolean, warning: boolean, providerId: string): void {
        if (this._crossed === crossed && this._warning === warning && this._providerId === providerId)
            return;
        this._crossed = crossed;
        this._warning = warning;
        this._providerId = providerId;
        this.queue_repaint();
    }

    _repaint(): void {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        const size = Math.min(width, height);
        const offsetX = (width - size) / 2;
        const offsetY = (height - size) / 2;
        const color = this.get_theme_node().get_foreground_color();
        const r = color.red / 255;
        const g = color.green / 255;
        const b = color.blue / 255;
        const a = color.alpha / 255;

        if (this._providerId === 'netbird') {
            const scale = size / Math.max(NETBIRD_VIEW.width, NETBIRD_VIEW.height);
            const left = offsetX + (size - NETBIRD_VIEW.width * scale) / 2;
            const top = offsetY + (size - NETBIRD_VIEW.height * scale) / 2;
            const at = (x: number, y: number): [number, number] =>
                [left + (x - NETBIRD_VIEW.x) * scale, top + (y - NETBIRD_VIEW.y) * scale];
            cr.setSourceRGBA(r, g, b, a);
            for (const path of NETBIRD_PATHS) {
                for (const step of path) {
                    if (step[0] === 'C')
                        cr.curveTo(...at(step[1], step[2]), ...at(step[3], step[4]), ...at(step[5], step[6]));
                    else if (step[0] === 'M')
                        cr.moveTo(...at(step[1], step[2]));
                    else
                        cr.lineTo(...at(step[1], step[2]));
                }
                cr.closePath();
                cr.fill();
            }
        } else if (this._providerId === 'zerotier') {
            cr.setSourceRGBA(r, g, b, a);
            drawZeroTier(cr, offsetX, offsetY, size);
        } else {
            const dot = Math.max(2, size * 0.24);
            const radius = dot / 2;
            const positions = [0, (size - dot) / 2, size - dot];
            const faded = [[0, 0], [1, 0], [2, 0], [0, 2], [2, 2]];
            const isFaded = (col: number, rowIndex: number) => faded.some(([c, rw]) => c === col && rw === rowIndex);

            for (let rowIndex = 0; rowIndex < 3; rowIndex++) {
                for (let col = 0; col < 3; col++) {
                    cr.setSourceRGBA(r, g, b, a * (isFaded(col, rowIndex) ? 0.24 : 1.0));
                    cr.arc(offsetX + positions[col] + radius, offsetY + positions[rowIndex] + radius,
                           radius, 0, 2 * Math.PI);
                    cr.fill();
                }
            }
        }

        if (this._crossed) {
            cr.setSourceRGBA(r, g, b, a);
            cr.setLineWidth(Math.max(2, size * 0.14));
            cr.setLineCap(1);
            const inset = size * 0.06;
            // NetBird's mark runs along the usual slash and would swallow it.
            const [from, to] = this._providerId === 'netbird' ? [inset, size - inset] : [size - inset, inset];
            cr.moveTo(offsetX + inset, offsetY + from);
            cr.lineTo(offsetX + size - inset, offsetY + to);
            cr.stroke();
        }

        if (this._warning) {
            const badge = Math.max(5, size * 0.42);
            cr.setSourceRGBA(0.95, 0.35, 0.35, 1.0);
            cr.arc(offsetX + size - badge / 2, offsetY + size - badge / 2, badge / 2, 0, 2 * Math.PI);
            cr.fill();
        }

        cr.$dispose();
    }
});

const Indicator = GObject.registerClass(
class TailGaugeIndicator extends PanelMenu.Button {
    declare _extension: TailGaugeExtension;
    declare _settings: Gio.Settings;
    declare _service: InstanceType<typeof ProviderService>;
    declare _signature: string;
    declare _phraseIndex: number;
    declare _phraseTimeoutId: number;
    declare _mullvadQuery: string;
    declare _machineQuery: string;
    declare _machineEntry: St.Entry | null;
    declare _mullvadEntry: St.Entry | null;
    declare _sections: Map<string, {header: PopupMenu.PopupSeparatorMenuItem; section: RowSection}>;
    declare _panelIcon: InstanceType<typeof TailGaugeIcon>;
    declare _panelCount: St.Label;
    declare _panelLabel: St.Label;
    declare _headerItem: PopupMenu.PopupSwitchMenuItem;
    declare _headerToggledId: number;
    declare _statusItem: PopupMenu.PopupMenuItem;
    declare _refreshItem: PopupMenu.PopupMenuItem;
    declare _footerItem: PopupMenu.PopupMenuItem;
    declare _scrolled: RowSection;
    declare _scroll: St.ScrollView;
    declare _changedId: number;
    declare _settingsPanelId: number;
    declare _keyFocusId: number;

    // PanelMenu.Button types `menu` as either the real menu or the dummy one;
    // a Button built with a menu always has the real one.
    get _menu(): PopupMenu.PopupMenu {
        return this.menu as PopupMenu.PopupMenu;
    }

    // The base class declares the shell's own construction overloads; GJS
    // routes `new Indicator(extension)` through the same method, so the
    // extension-shaped call is declared alongside them.
    override _init(params?: Partial<PanelMenu.ButtonBox.ConstructorProps>): void;
    override _init(menuAlignment: number, nameText: string, dontCreateMenu?: boolean): void;
    override _init(extension: TailGaugeExtension): void;
    override _init(extension: any): void {
        super._init(0.5, 'TailGauge');

        this._extension = extension;
        this._settings = extension.getSettings();
        this._service = new ProviderService(this._settings,
            String(extension.metadata['version-name'] ?? ''));
        this._signature = '';
        this._phraseIndex = 0;
        this._pushUi();
        this._phraseTimeoutId = 0;
        this._mullvadQuery = '';
        this._machineQuery = '';
        this._machineEntry = null;
        this._mullvadEntry = null;
        this._sections = new Map<string, {header: PopupMenu.PopupSeparatorMenuItem; section: RowSection}>();

        const panelBox = box(false, {style_class: 'panel-status-menu-box tailgauge-panel'});
        this._panelIcon = new TailGaugeIcon({width: 16, height: 16, y_align: Clutter.ActorAlign.CENTER});
        this._panelCount = new St.Label({
            style_class: 'tailgauge-panel-count',
            y_align: Clutter.ActorAlign.START,
            visible: false,
        });
        this._panelLabel = new St.Label({
            style_class: 'tailgauge-panel-label',
            y_align: Clutter.ActorAlign.CENTER,
        });
        panelBox.add_child(this._panelIcon);
        panelBox.add_child(this._panelCount);
        panelBox.add_child(this._panelLabel);
        this.add_child(panelBox);

        this._buildStaticItems();

        this._changedId = this._service.connect('changed', () => this._sync());
        this._settingsPanelId = this._settings.connect('changed::show-status-in-panel',
            () => this._syncPanel(this._panel()));

        this._menu.connect('open-state-changed', (_menu, open) => {
            this._service.attentive = open;
            if (open) {
                this._updateScrollHeight();
                verticalAdjustment(this._scroll).value = 0;
                this._service.refresh();
                this._startPhrases();
            } else {
                this._stopPhrases();
                this._machineEntry?.set_text('');
                this._mullvadEntry?.set_text('');
            }
        });

        this.connect('button-press-event', (_actor, event) => {
            if (event.get_button() === Clutter.BUTTON_MIDDLE) {
                this._cycleProvider();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        this._menu.box.connect('key-press-event', (_actor, event) => this._onMenuKey(event));

        this._keyFocusId = global.stage.connect('notify::key-focus',
            () => this._scrollFocusIntoView());

        this._sync();
    }

    // The panel arrives resolved from the binary, so what GNOME shows and what
    // the other desktops show cannot drift. The picker is always asked for
    // open: its regions live in a submenu that PopupMenu shows and hides on
    // its own.
    _panel(): Panel.Panel {
        return this._service.panel;
    }

    // The switcher section is the drivable providers, in order, so the next one
    // is the row after the current one.
    _cycleProvider(): void {
        const rows = this._panel().sections.find(s => s.id === 'providers')?.rows ?? [];
        if (rows.length < 2)
            return;
        const at = rows.findIndex(row => row.current);
        this._service.switchProvider(rows[(at + 1) % rows.length].payload);
    }

    // What only this side decides. Assigning it makes the service re-ask, so a
    // phrase tick or a newly expanded row redraws rather than waiting.
    _pushUi(): void {
        this._service.ui = {
            recentRegions: this._settings.get_strv('recent-mullvad-regions'),
            mullvadPickerOpen: true,
            phraseIndex: this._phraseIndex,
        };
    }

    _buildStaticItems(): void {
        this._headerItem = new PopupMenu.PopupSwitchMenuItem('Tailscale', false);
        this._headerToggledId = this._headerItem.connect('toggled',
            () => this._service.toggleTailscale());
        this._menu.addMenuItem(this._headerItem);

        this._statusItem = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
        this._statusItem.label.add_style_class_name('tailgauge-status');
        this._statusItem.label.clutter_text.line_wrap = true;
        this._menu.addMenuItem(this._statusItem);

        // The shell keeps a tall menu on screen by moving it, never by
        // shrinking it, so the sections that grow with the tailnet carry their
        // own scroll view; Refresh and Settings stay below it either way.
        this._scrolled = new PopupMenu.PopupMenuSection();
        this._menu.addMenuItem(this._scrolled);
        this._menu.box.remove_child(this._scrolled.actor);
        this._scroll = scrollView(this._scrolled.actor, {
            style_class: 'tailgauge-scroll',
            x_expand: true,
            clip_to_allocation: true,
        });
        (this._scroll as unknown as Loose)._delegate = this._scrolled;
        this._menu.box.add_child(this._scroll);

        // Every section the binary emits, in the order it emits them. A
        // section with no slot here is dropped by _rebuildSections, which is
        // how provider switching and network selection went missing on GNOME
        // while their actions sat in the dispatch unreachable.
        for (const id of Panel.SECTION_IDS) {
            const header = new PopupMenu.PopupSeparatorMenuItem('');
            const section = new PopupMenu.PopupMenuSection();
            this._scrolled.addMenuItem(header);
            this._scrolled.addMenuItem(section);
            this._sections.set(id, {header, section});
        }

        this._menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        // Labelled from the panel, not from here, so every desktop names it
        // the same way and it is translated once.
        this._refreshItem = new PopupMenu.PopupMenuItem('');
        this._refreshItem.connect('activate', () => this._service.refresh());
        this._menu.addMenuItem(this._refreshItem);

        const settingsItem = new PopupMenu.PopupMenuItem(_('Settings'));
        settingsItem.connect('activate', () => this._extension.openPreferences());
        this._menu.addMenuItem(settingsItem);

        this._footerItem = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
        this._footerItem.label.add_style_class_name('tailgauge-footer');
        this._menu.addMenuItem(this._footerItem);
    }

    // ---- scrolling -------------------------------------------------------

    // Nothing hands a menu a height budget, so the scroll view takes its share
    // of the work area, resolved on every open in case the monitor changed.
    _updateScrollHeight(): void {
        const monitor = Main.layoutManager.findMonitorForActor(this) ??
            Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        const workArea = Main.layoutManager.getWorkAreaForMonitor(monitor.index);
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const limit = Math.max(SCROLL_MIN_HEIGHT,
            Math.round(workArea.height * SCROLL_WORK_AREA_SHARE / scale));
        this._scroll.style = `max-height: ${limit}px`;
    }

    // PopupMenu walks key focus through the rows knowing nothing about the
    // scroll view, so a row below the fold would be focused off screen.
    _scrollFocusIntoView(): void {
        const focus = global.stage.get_key_focus();
        if (!focus || focus === this._scroll || !this._scroll.contains(focus))
            return;
        ensureActorVisibleInScrollView(this._scroll, focus);
    }

    // ---- sync ------------------------------------------------------------

    _sync(): void {
        const panel = this._panel();
        this._syncPanel(panel);
        this._syncHeader(panel);

        // A rebuild throws away hover and key focus, so it only happens when
        // the set of rows actually changed, not on every poll.
        const signature = this._signatureOf(panel);
        if (signature !== this._signature) {
            this._signature = signature;
            this._rebuildSections(panel);
        } else {
            this._syncRows(panel);
        }
    }

    _signatureOf(panel: Panel.Panel): string {
        const parts = [panel.header.toggleVisible ? '1' : '0'];
        for (const section of panel.sections) {
            parts.push(`${section.id}:${section.visible ? 1 : 0}:` +
                section.rows.map(r => r.id + '/' + r.children.map(c => c.id).join('~')).join(','));
        }
        return parts.join('|');
    }

    _syncPanel(panel: Panel.Panel): void {
        // The bar describes the machine, not the panel's current view.
        this._panelIcon.setState(panel.bar.crossed, panel.bar.warning, panel.bar.providerId);
        this._panelIcon.opacity = panel.bar.connected ? 255 : 130;
        this._panelCount.text = panel.bar.badge;
        this._panelCount.visible = panel.bar.badge !== '';

        const showName = this._settings.get_boolean('show-status-in-panel');
        const name = panel.header.toggleVisible ? panel.header.title : '';
        this._panelLabel.text = showName && name ? ` ${name}` : '';
        this._panelLabel.visible = this._panelLabel.text !== '';
    }

    _syncHeader(panel: Panel.Panel): void {
        this._headerItem.label.text = panel.header.title;
        this._headerItem.setSensitive(panel.header.toggleEnabled);
        // GNOME 47+ emits 'toggled' from setToggleState too, which would turn
        // mirroring the daemon into a click: an on/off loop.
        if (this._headerItem.state !== panel.header.toggleChecked) {
            this._headerItem.block_signal_handler(this._headerToggledId);
            try {
                this._headerItem.setToggleState(panel.header.toggleChecked);
            } finally {
                this._headerItem.unblock_signal_handler(this._headerToggledId);
            }
        }

        // The idle line is the header's own meta; the status slot carries only
        // what the model put there.
        const text = panel.status.text !== '' ? panel.status.text : panel.header.meta;
        this._statusItem.label.text = text;
        this._statusItem.visible = text !== '';
        if (panel.status.tone === 'error')
            this._statusItem.label.add_style_class_name('tailgauge-error');
        else
            this._statusItem.label.remove_style_class_name('tailgauge-error');

        const refresh = panel.header.actions.find(a => a.id === 'refresh');
        this._refreshItem.label.text = refresh ? refresh.label : '';
        this._refreshItem.visible = refresh !== undefined;
        this._refreshItem.setSensitive(panel.header.toggleVisible);

        this._footerItem.label.text = panel.footer;
        this._footerItem.visible = panel.footer !== '';
    }

    // Cheap pass for the things that change without the row set changing.
    _syncRows(panel: Panel.Panel): void {
        const byId = new Map<string, Panel.PanelRow>();
        for (const section of panel.sections) {
            for (const row of section.rows) {
                byId.set(row.id, row);
                for (const child of row.children)
                    byId.set(child.id, child);
            }
        }
        for (const {section} of this._sections.values()) {
            for (const item of menuItems(section))
                this._applyRow(item, byId);
        }
    }

    _applyRow(item: RowItem, byId: Map<string, Panel.PanelRow>): void {
        const row = item._rowId ? byId.get(item._rowId) : null;
        if (row) {
            item.setOrnament(row.current ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
            if (item.label)
                item.label.text = row.label;
            if (item._sublabel)
                item._sublabel.text = row.sublabel;
        }
        if (item.menu) {
            for (const child of menuItems(item.menu))
                this._applyRow(child, byId);
        }
    }

    // The model decides what a row is findable by and hands it over as a
    // pre-lowercased key; the only thing left here is the substring test. The
    // rows are built once and hidden, never destroyed, so the entry being typed
    // into survives the query that changes what is on screen.
    _searchQuery(scope: string): string {
        if (scope === 'machines')
            return this._machineQuery.trim().toLowerCase();
        if (scope === 'mullvad')
            return this._mullvadQuery.trim().toLowerCase();
        return '';
    }

    _searchMatches(row: Panel.PanelRow): boolean {
        const query = this._searchQuery(row.searchScope);
        return query === '' || row.searchKey.includes(query);
    }

    // A scope whose query matches nothing draws its keyless row instead, which
    // is the message the model wrote for exactly that case.
    _applySearch(): void {
        const filtered: {item: RowItem; row: Panel.PanelRow}[] = [];
        const matched = new Set<string>();

        const collect = (items: RowItem[]): void => {
            for (const item of items) {
                const row = item._row;
                if (row && row.searchScope !== '') {
                    filtered.push({item, row});
                    if (row.searchKey !== '' && this._searchMatches(row))
                        matched.add(row.searchScope);
                }
                if (item.menu)
                    collect(menuItems(item.menu));
            }
        };
        for (const {section} of this._sections.values())
            collect(menuItems(section));

        for (const {item, row} of filtered) {
            item.visible = row.searchKey !== ''
                ? this._searchMatches(row)
                : this._searchQuery(row.searchScope) !== '' && !matched.has(row.searchScope);
        }
    }

    _rebuildSections(panel: Panel.Panel): void {
        // Both fields are about to be destroyed with the sections holding them.
        this._machineEntry = null;
        this._mullvadEntry = null;
        for (const section of panel.sections) {
            const slot = this._sections.get(section.id);
            if (!slot)
                continue;
            slot.header.label.text = section.title;
            slot.header.visible = section.visible && section.title !== '';
            slot.section.removeAll();
            slot.section.actor.visible = section.visible;

            if (section.rows.length === 0 && section.empty !== '' && section.visible) {
                slot.section.addMenuItem(new PopupMenu.PopupMenuItem(
                    section.empty, {reactive: false, can_focus: false}));
                continue;
            }
            for (const row of section.rows)
                slot.section.addMenuItem(this._renderRow(row));
        }
        // A query left behind by the field that set it would filter a list with
        // nothing on screen left to clear it.
        if (!this._machineEntry)
            this._machineQuery = '';
        if (!this._mullvadEntry)
            this._mullvadQuery = '';
        this._applySearch();
    }

    _renderRow(row: Panel.PanelRow): RowItem {
        if (row.kind === 'empty') {
            const empty: RowItem = new PopupMenu.PopupMenuItem(
                row.label, {reactive: false, can_focus: false});
            empty._rowId = row.id;
            empty._row = row;
            return empty;
        }

        if (row.kind === 'machineSearch')
            return this._renderMachineSearch(row);

        if (row.children.length > 0 || row.copyOptions.length > 0 || row.actions.length > 0)
            return this._renderSubmenuRow(row);

        const item = new PopupMenu.PopupMenuItem(row.label);
        this._decorate(item, row);
        item.setOrnament(row.current ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        item.connect('activate', () => this._dispatch(row));
        return item;
    }

    _renderMachineSearch(row: Panel.PanelRow): RowItem {
        const item: RowItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        item._rowId = row.id;
        item._row = row;
        const entry = new St.Entry({
            style_class: 'tailgauge-search',
            hint_text: row.searchPlaceholder,
            can_focus: true,
            x_expand: true,
        });
        entry.set_text(this._machineQuery);
        entry.clutter_text.connect('text-changed', () => {
            this._machineQuery = entry.get_text();
            this._applySearch();
        });
        item.add_child(entry);
        this._machineEntry = entry;
        return item;
    }

    _renderSubmenuRow(row: Panel.PanelRow): RowItem {
        const item = new PopupMenu.PopupSubMenuMenuItem(row.label, true);
        item.icon!.icon_name = row.icon;
        this._decorate(item, row);

        // The picker's own row does nothing but hold its regions; every other
        // submenu row still carries its primary action as the first entry.
        if (row.kind === 'mullvadPicker') {
            const searchItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
            const entry = new St.Entry({
                style_class: 'tailgauge-search',
                hint_text: row.searchPlaceholder,
                can_focus: true,
                x_expand: true,
            });
            entry.set_text(this._mullvadQuery);
            entry.clutter_text.connect('text-changed', () => {
                this._mullvadQuery = entry.get_text();
                this._applySearch();
            });
            searchItem.add_child(entry);
            this._mullvadEntry = entry;
            item.menu.addMenuItem(searchItem);
            item.menu.connect('open-state-changed', (_menu, open) => {
                if (!open)
                    return;
                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    entry.grab_key_focus();
                    return GLib.SOURCE_REMOVE;
                });
            });
        }

        for (const option of row.copyOptions) {
            const copyItem = new PopupMenu.PopupMenuItem(option.label);
            copyItem.add_child(new St.Icon({
                icon_name: 'edit-copy-symbolic',
                style_class: 'popup-menu-icon tailgauge-copy-icon',
                x_expand: true,
                x_align: Clutter.ActorAlign.END,
            }));
            copyItem.connect('activate', () => {
                this._copyOption(row, option.kind);
                this._menu.close();
            });
            item.menu.addMenuItem(copyItem);
        }

        for (const action of row.actions) {
            if (action.id !== 'send')
                continue;
            item.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
            const sendItem = new PopupMenu.PopupMenuItem(action.label);
            sendItem.connect('activate', () => this._sendFile(row));
            item.menu.addMenuItem(sendItem);
        }

        for (const child of row.children)
            item.menu.addMenuItem(this._renderRow(child));

        return item;
    }

    // The sublabel rides on the trailing edge of the row rather than under it:
    // a PopupMenuItem is one line tall, and dropping it instead would put GNOME
    // and Plasma back out of step.
    _decorate(item: RowItem, row: Panel.PanelRow): void {
        item._rowId = row.id;
        item._row = row;
        if (item.label)
            item.label.text = row.label;
        if (row.sublabel === '')
            return;
        item._sublabel = new St.Label({
            text: row.sublabel,
            style_class: 'tailgauge-sublabel',
            x_expand: true,
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.CENTER,
        });
        item.add_child(item._sublabel);
    }

    // ---- actions ---------------------------------------------------------

    toggle(): void {
        this._service.toggleTailscale();
    }

    // The one place a resolved row turns back into a service call.
    _dispatch(row: Panel.PanelRow | null | undefined): void {
        if (!row)
            return;
        switch (row.action) {
        case 'toggle':
            this._service.toggleTailscale();
            break;
        case 'authorize':
            this._service.authorizeOperator();
            break;
        case 'switchProvider':
            this._service.switchProvider(row.payload);
            break;
        case 'selectNetwork':
            this._service.selectNetwork(row.payload);
            break;
        case 'switchAccount':
            this._service.switchAccount(row.payload?.id ?? '');
            break;
        case 'update':
            this._service.applyUpdate();
            this._menu.close();
            break;
        case 'openUrl':
            this._service.openUrl(row.payload?.url ?? '');
            this._menu.close();
            break;
        case 'setExitNode':
            if (row.payload?.Mullvad === true)
                this._service.rememberMullvadRegion(row.payload);
            this._service.setExitNode(row.payload);
            break;
        }
    }

    _copyOption(row: Panel.PanelRow | null | undefined, kind: string): void {
        if (!row)
            return;
        if (kind === 'name')
            this._service.copyPeerName(row.payload);
        else if (kind === 'dns')
            this._service.copyPeerDnsName(row.payload);
        else if (kind === 'ip')
            this._service.copyPeerIp(row.payload);
        else
            for (const option of row.copyOptions)
                if (option.kind === kind)
                    this._service.copyToClipboard(option.label);
    }

    _sendFile(row: Panel.PanelRow | null | undefined): void {
        if (!row || !row.payload)
            return;
        // The file chooser takes over from here, so get the menu out of the way.
        this._service.sendFile(row.payload);
        this._menu.close();
    }

    // ---- keyboard --------------------------------------------------------

    // PopupMenu already handles arrows and Enter; these are the single-letter
    // actions the Omarchy panel binds, resolved against whichever row currently
    // holds key focus.
    _onMenuKey(event: Clutter.Event): boolean {
        const focus = global.stage.get_key_focus();
        if (focus instanceof Clutter.Text && focus.editable)
            return Clutter.EVENT_PROPAGATE;

        const symbol = event.get_key_symbol();

        if (symbol === Clutter.KEY_t || symbol === Clutter.KEY_T) {
            this._service.toggleTailscale();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_r || symbol === Clutter.KEY_R) {
            this._service.refresh();
            return Clutter.EVENT_STOP;
        }

        const row = this._focusedRow();
        if (!row)
            return Clutter.EVENT_PROPAGATE;

        const copyable = row.copyOptions.length > 0;
        if (copyable && (symbol === Clutter.KEY_c || symbol === Clutter.KEY_C))
            this._copyOption(row, 'ip');
        else if (copyable && (symbol === Clutter.KEY_n || symbol === Clutter.KEY_N))
            this._copyOption(row, 'name');
        else if (copyable && (symbol === Clutter.KEY_d || symbol === Clutter.KEY_D))
            this._copyOption(row, 'dns');
        else if ((symbol === Clutter.KEY_s || symbol === Clutter.KEY_S) &&
                 row.actions.some(action => action.id === 'send'))
            this._sendFile(row);
        else
            return Clutter.EVENT_PROPAGATE;

        if (symbol !== Clutter.KEY_s && symbol !== Clutter.KEY_S)
            this._menu.close();
        return Clutter.EVENT_STOP;
    }

    _focusedRow(): Panel.PanelRow | null {
        let actor = global.stage.get_key_focus() as Delegated | null;
        while (actor) {
            if (actor._delegate?._row)
                return actor._delegate._row;
            if (actor._row)
                return actor._row;
            actor = actor.get_parent() as Delegated | null;
        }
        for (const {section} of this._sections.values()) {
            for (const item of menuItems(section))
                if (item.active && item._row)
                    return item._row;
        }
        return null;
    }

    // ---- phrases ---------------------------------------------------------

    _startPhrases(): void {
        this._stopPhrases();
        this._phraseTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PHRASE_INTERVAL_MS, () => {
            this._phraseIndex += 1;
            // The phrase is the binary's to pick, so the tick asks rather than
            // rotating a list of its own.
            this._pushUi();
            return GLib.SOURCE_CONTINUE;
        });
    }

    _stopPhrases(): void {
        if (this._phraseTimeoutId) {
            GLib.Source.remove(this._phraseTimeoutId);
            this._phraseTimeoutId = 0;
        }
    }

    override destroy(): void {
        this._stopPhrases();
        if (this._keyFocusId) {
            global.stage.disconnect(this._keyFocusId);
            this._keyFocusId = 0;
        }
        if (this._changedId) {
            this._service.disconnect(this._changedId);
            this._changedId = 0;
        }
        if (this._settingsPanelId) {
            this._settings.disconnect(this._settingsPanelId);
            this._settingsPanelId = 0;
        }
        this._service.destroy();
        super.destroy();
    }
});

export default class TailGaugeExtension extends Extension {
    declare _indicator: InstanceType<typeof Indicator> | null;
    declare _settings: Gio.Settings | null;

    override enable(): void {
        this._indicator = new Indicator(this);
        Main.panel.addToStatusArea(this.uuid, this._indicator as unknown as PanelMenu.Button);

        // The setting is empty by default, so this registers nothing until
        // asked.
        this._settings = this.getSettings();
        Main.wm.addKeybinding('toggle-shortcut', this._settings,
            Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW,
            () => this._indicator?.toggle());
    }

    override disable(): void {
        Main.wm.removeKeybinding('toggle-shortcut');
        this._settings = null;
        this._indicator?.destroy();
        this._indicator = null;
    }
}
