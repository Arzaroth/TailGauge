import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {ensureActorVisibleInScrollView} from 'resource:///org/gnome/shell/misc/animationUtils.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import * as Model from './model.js';
import {TailscaleService} from './tailscale.js';

const RECENT_MULLVAD_LIMIT = 5;
const PHRASE_INTERVAL_MS = 2800;
const MULLVAD_REGION_CAP = 200;
const SCROLL_WORK_AREA_SHARE = 0.6;
const SCROLL_MIN_HEIGHT = 200;

// St.BoxLayout.vertical was replaced by the Clutter orientation property in
// GNOME 48; both spellings have to work across the supported shell versions.
function box(vertical, props = {}) {
    const b = new St.BoxLayout(props);
    if ('orientation' in b)
        b.orientation = vertical ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL;
    else
        b.vertical = vertical;
    return b;
}

// St.ScrollView took its content through add_actor until GNOME 46 turned it
// into a property, and only gained an adjustment of its own in that release.
function scrollView(child, props = {}) {
    const view = new St.ScrollView({
        hscrollbar_policy: St.PolicyType.NEVER,
        vscrollbar_policy: St.PolicyType.AUTOMATIC,
        ...props,
    });
    if ('child' in view)
        view.child = child;
    else
        view.add_actor(child);
    return view;
}

function verticalAdjustment(view) {
    return 'vadjustment' in view ? view.vadjustment : view.vscroll.adjustment;
}

// Native rendering of the Tailscale mark from the SVG: a 3x3 dot grid with the
// inactive dots faded, plus the disconnected slash and the needs-login badge.
const TailscaleIcon = GObject.registerClass(
class TailscaleIcon extends St.DrawingArea {
    _init(params = {}) {
        super._init({
            style_class: 'system-status-icon tailgauge-icon',
            ...params,
        });
        this._crossed = false;
        this._warning = false;
        this.connect('repaint', () => this._repaint());
    }

    setState(crossed, warning) {
        if (this._crossed === crossed && this._warning === warning)
            return;
        this._crossed = crossed;
        this._warning = warning;
        this.queue_repaint();
    }

    _repaint() {
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

        const dot = Math.max(2, size * 0.24);
        const radius = dot / 2;
        const positions = [0, (size - dot) / 2, size - dot];
        const faded = [[0, 0], [1, 0], [2, 0], [0, 2], [2, 2]];
        const isFaded = (col, rowIndex) => faded.some(([c, rw]) => c === col && rw === rowIndex);

        for (let rowIndex = 0; rowIndex < 3; rowIndex++) {
            for (let col = 0; col < 3; col++) {
                cr.setSourceRGBA(r, g, b, a * (isFaded(col, rowIndex) ? 0.24 : 1.0));
                cr.arc(offsetX + positions[col] + radius, offsetY + positions[rowIndex] + radius,
                       radius, 0, 2 * Math.PI);
                cr.fill();
            }
        }

        if (this._crossed) {
            cr.setSourceRGBA(r, g, b, a);
            cr.setLineWidth(Math.max(2, size * 0.14));
            cr.setLineCap(1);
            const inset = size * 0.06;
            cr.moveTo(offsetX + inset, offsetY + size - inset);
            cr.lineTo(offsetX + size - inset, offsetY + inset);
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
    _init(extension) {
        super._init(0.5, 'TailGauge');

        this._extension = extension;
        this._settings = extension.getSettings();
        this._service = new TailscaleService(this._settings);
        this._signature = '';
        this._phraseIndex = 0;
        this._phraseTimeoutId = 0;
        this._mullvadQuery = '';
        this._sections = new Map();

        const panelBox = box(false, {style_class: 'panel-status-menu-box tailgauge-panel'});
        this._panelIcon = new TailscaleIcon({width: 16, height: 16, y_align: Clutter.ActorAlign.CENTER});
        this._panelLabel = new St.Label({
            style_class: 'tailgauge-panel-label',
            y_align: Clutter.ActorAlign.CENTER,
        });
        panelBox.add_child(this._panelIcon);
        panelBox.add_child(this._panelLabel);
        this.add_child(panelBox);

        this._buildStaticItems();

        this._changedId = this._service.connect('changed', () => this._sync());
        this._settingsPanelId = this._settings.connect('changed::show-status-in-panel',
            () => this._syncPanel(this._panel()));

        this.menu.connect('open-state-changed', (_menu, open) => {
            this._service.attentive = open;
            if (open) {
                this._updateScrollHeight();
                verticalAdjustment(this._scroll).value = 0;
                this._service.refresh();
                this._startPhrases();
            } else {
                this._stopPhrases();
                if (this._mullvadQuery !== '') {
                    this._mullvadQuery = '';
                    this._signature = '';
                }
            }
        });

        this.connect('button-press-event', (_actor, event) => {
            if (event.get_button() === Clutter.BUTTON_MIDDLE) {
                this._service.toggleTailscale();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        this.menu.box.connect('key-press-event', (_actor, event) => this._onMenuKey(event));

        this._keyFocusId = global.stage.connect('notify::key-focus',
            () => this._scrollFocusIntoView());

        this._sync();
    }

    // The panel is resolved in the shared model, so what GNOME shows and what
    // Plasma shows cannot drift. The picker is always resolved open here: its
    // regions live in a submenu that PopupMenu shows and hides on its own.
    _panel() {
        return Model.resolvePanel(this._service.snapshot(), {
            t: _,
            recentRegions: this._settings.get_strv('recent-mullvad-regions'),
            mullvadQuery: this._mullvadQuery,
            mullvadPickerOpen: true,
            phraseIndex: this._phraseIndex,
        });
    }

    _buildStaticItems() {
        this._headerItem = new PopupMenu.PopupSwitchMenuItem('Tailscale', false);
        this._headerItem.connect('toggled', () => this._service.toggleTailscale());
        this.menu.addMenuItem(this._headerItem);

        this._statusItem = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
        this._statusItem.label.add_style_class_name('tailgauge-status');
        this._statusItem.label.clutter_text.line_wrap = true;
        this.menu.addMenuItem(this._statusItem);

        // The shell keeps a tall menu on screen by moving it, never by
        // shrinking it, so the sections that grow with the tailnet carry their
        // own scroll view; Refresh and Settings stay below it either way.
        this._scrolled = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this._scrolled);
        this.menu.box.remove_child(this._scrolled.actor);
        this._scroll = scrollView(this._scrolled.actor, {
            style_class: 'tailgauge-scroll',
            x_expand: true,
            clip_to_allocation: true,
        });
        this._scroll._delegate = this._scrolled;
        this.menu.box.add_child(this._scroll);

        for (const id of ['update', 'self', 'connections', 'exitNodes', 'machines']) {
            const header = new PopupMenu.PopupSeparatorMenuItem('');
            const section = new PopupMenu.PopupMenuSection();
            this._scrolled.addMenuItem(header);
            this._scrolled.addMenuItem(section);
            this._sections.set(id, {header, section});
        }

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        this._refreshItem = new PopupMenu.PopupMenuItem(_('Refresh'));
        this._refreshItem.connect('activate', () => this._service.refresh(true));
        this.menu.addMenuItem(this._refreshItem);

        const settingsItem = new PopupMenu.PopupMenuItem(_('Settings'));
        settingsItem.connect('activate', () => this._extension.openPreferences());
        this.menu.addMenuItem(settingsItem);
    }

    // ---- scrolling -------------------------------------------------------

    // Nothing hands a menu a height budget, so the scroll view takes its share
    // of the work area, resolved on every open in case the monitor changed.
    _updateScrollHeight() {
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
    _scrollFocusIntoView() {
        const focus = global.stage.get_key_focus();
        if (!focus || focus === this._scroll || !this._scroll.contains(focus))
            return;
        ensureActorVisibleInScrollView(this._scroll, focus);
    }

    // ---- sync ------------------------------------------------------------

    _sync() {
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

    _signatureOf(panel) {
        const parts = [panel.header.toggleVisible ? '1' : '0'];
        for (const section of panel.sections) {
            parts.push(`${section.id}:${section.visible ? 1 : 0}:` +
                section.rows.map(r => r.id + '/' + r.children.map(c => c.id).join('~')).join(','));
        }
        return parts.join('|');
    }

    _syncPanel(panel) {
        this._panelIcon.setState(panel.header.crossed, panel.header.warning);
        this._panelIcon.opacity = panel.header.dimmed ? 130 : 255;

        const showName = this._settings.get_boolean('show-status-in-panel');
        const name = panel.header.toggleVisible ? panel.header.title : '';
        this._panelLabel.text = showName && name ? ` ${name}` : '';
        this._panelLabel.visible = this._panelLabel.text !== '';
    }

    _syncHeader(panel) {
        this._headerItem.label.text = panel.header.title;
        this._headerItem.setSensitive(panel.header.toggleEnabled);
        if (this._headerItem.state !== panel.header.toggleChecked)
            this._headerItem.setToggleState(panel.header.toggleChecked);

        // The idle line is the header's own meta; the status slot carries only
        // what the model put there.
        const text = panel.status.text !== '' ? panel.status.text : panel.header.meta;
        this._statusItem.label.text = text;
        this._statusItem.visible = text !== '';
        if (panel.status.tone === 'error')
            this._statusItem.label.add_style_class_name('tailgauge-error');
        else
            this._statusItem.label.remove_style_class_name('tailgauge-error');

        this._refreshItem.setSensitive(panel.header.toggleVisible);
    }

    // Cheap pass for the things that change without the row set changing.
    _syncRows(panel) {
        const byId = new Map();
        for (const section of panel.sections) {
            for (const row of section.rows) {
                byId.set(row.id, row);
                for (const child of row.children)
                    byId.set(child.id, child);
            }
        }
        for (const {section} of this._sections.values()) {
            for (const item of section._getMenuItems())
                this._applyRow(item, byId);
        }
    }

    _applyRow(item, byId) {
        const row = item._rowId ? byId.get(item._rowId) : null;
        if (row) {
            item.setOrnament(row.current ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
            if (item.label)
                item.label.text = row.label;
            if (item._sublabel)
                item._sublabel.text = row.sublabel;
        }
        if (item.menu) {
            for (const child of item.menu._getMenuItems())
                this._applyRow(child, byId);
        }
    }

    _rebuildSections(panel) {
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
    }

    _renderRow(row) {
        if (row.kind === 'empty')
            return new PopupMenu.PopupMenuItem(row.label, {reactive: false, can_focus: false});

        if (row.children.length > 0 || row.copyOptions.length > 0 || row.actions.length > 0)
            return this._renderSubmenuRow(row);

        const item = new PopupMenu.PopupMenuItem(row.label);
        this._decorate(item, row);
        item.setOrnament(row.current ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        item.connect('activate', () => this._dispatch(row));
        return item;
    }

    _renderSubmenuRow(row) {
        const item = new PopupMenu.PopupSubMenuMenuItem(row.label, true);
        item.icon.icon_name = row.icon;
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
                this._refillPicker(item, entry);
            });
            searchItem.add_child(entry);
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
                this.menu.close();
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
    _decorate(item, row) {
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

    _refillPicker(item, entry) {
        const panel = this._panel();
        let picker = null;
        for (const section of panel.sections) {
            for (const candidate of section.rows) {
                if (candidate.kind === 'mullvadPicker')
                    picker = candidate;
            }
        }
        if (!picker)
            return;
        // Everything after the search entry is a resolved region row.
        const items = item.menu._getMenuItems();
        for (const child of items.slice(1))
            child.destroy();
        for (const child of picker.children.slice(0, MULLVAD_REGION_CAP))
            item.menu.addMenuItem(this._renderRow(child));
        entry.grab_key_focus();
    }

    // ---- actions ---------------------------------------------------------

    // The one place a resolved row turns back into a service call.
    _dispatch(row) {
        if (!row)
            return;
        switch (row.action) {
        case 'toggle':
            this._service.toggleTailscale();
            break;
        case 'authorize':
            this._service.authorizeTailscaleOperator();
            break;
        case 'switchAccount':
            this._service.switchAccount(row.payload.id);
            break;
        case 'update':
            this._service.applyUpdate();
            this.menu.close();
            break;
        case 'openUrl':
            this._service.openUrl(row.payload ? row.payload.url : '');
            this.menu.close();
            break;
        case 'setExitNode':
            if (row.payload.Mullvad === true) {
                this._settings.set_strv('recent-mullvad-regions', Model.pushRecentMullvad(
                    this._settings.get_strv('recent-mullvad-regions'),
                    Model.mullvadRegionKey(row.payload),
                    RECENT_MULLVAD_LIMIT));
            }
            this._service.setExitNode(row.payload);
            break;
        }
    }

    _copyOption(row, kind) {
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

    _sendFile(row) {
        if (!row || !row.payload)
            return;
        // The file chooser takes over from here, so get the menu out of the way.
        this._service.sendFile(row.payload);
        this.menu.close();
    }

    // ---- keyboard --------------------------------------------------------

    // PopupMenu already handles arrows and Enter; these are the single-letter
    // actions the Omarchy panel binds, resolved against whichever row currently
    // holds key focus.
    _onMenuKey(event) {
        const focus = global.stage.get_key_focus();
        if (focus instanceof Clutter.Text && focus.editable)
            return Clutter.EVENT_PROPAGATE;

        const symbol = event.get_key_symbol();

        if (symbol === Clutter.KEY_t || symbol === Clutter.KEY_T) {
            this._service.toggleTailscale();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_r || symbol === Clutter.KEY_R) {
            this._service.refresh(true);
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
                 Model.panelRowHasAction(row, 'send'))
            this._sendFile(row);
        else
            return Clutter.EVENT_PROPAGATE;

        if (symbol !== Clutter.KEY_s && symbol !== Clutter.KEY_S)
            this.menu.close();
        return Clutter.EVENT_STOP;
    }

    _focusedRow() {
        let actor = global.stage.get_key_focus();
        while (actor) {
            if (actor._delegate?._row)
                return actor._delegate._row;
            if (actor._row)
                return actor._row;
            actor = actor.get_parent();
        }
        for (const {section} of this._sections.values()) {
            for (const item of section._getMenuItems())
                if (item.active && item._row)
                    return item._row;
        }
        return null;
    }

    // ---- phrases ---------------------------------------------------------

    _startPhrases() {
        this._stopPhrases();
        this._phraseTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PHRASE_INTERVAL_MS, () => {
            this._phraseIndex += 1;
            this._syncHeader(this._panel());
            return GLib.SOURCE_CONTINUE;
        });
    }

    _stopPhrases() {
        if (this._phraseTimeoutId) {
            GLib.Source.remove(this._phraseTimeoutId);
            this._phraseTimeoutId = 0;
        }
    }

    destroy() {
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
    enable() {
        this._indicator = new Indicator(this);
        Main.panel.addToStatusArea(this.uuid, this._indicator);
    }

    disable() {
        this._indicator?.destroy();
        this._indicator = null;
    }
}
