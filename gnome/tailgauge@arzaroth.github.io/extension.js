import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import * as Model from './model.js';
import {TailscaleService} from './tailscale.js';

const RECENT_MULLVAD_LIMIT = 5;
const PHRASE_INTERVAL_MS = 2800;

const ACTIVE_PHRASES = [
    'Encrypting connections',
    'Sending secrets',
    'Guarding wires',
    'Braiding packets',
    'Polishing tunnels',
    'Hiding routes',
    'Sealing ports',
    'Sorting tailnets',
    'Shuffling keys',
    'Watching machines',
];

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
        const faded = [
            [0, 0], [1, 0], [2, 0],
            [0, 2], [2, 2],
        ];
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
            const cx = offsetX + size - badge / 2;
            const cy = offsetY + size - badge / 2;
            cr.setSourceRGBA(0.95, 0.35, 0.35, 1.0);
            cr.arc(cx, cy, badge / 2, 0, 2 * Math.PI);
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
        this._peerItems = [];

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
            () => this._syncPanel());

        this.menu.connect('open-state-changed', (_menu, open) => {
            if (open) {
                this._service.refresh();
                this._startPhrases();
            } else {
                this._stopPhrases();
                this._mullvadQuery = '';
            }
        });

        this.connect('button-press-event', (_actor, event) => {
            if (event.get_button() === Clutter.BUTTON_MIDDLE) {
                this._service.toggleTailscale();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });

        this.menu.box.connect('key-press-event', (actor, event) => this._onMenuKey(event));

        this._sync();
    }

    // ---- static scaffolding ---------------------------------------------

    _buildStaticItems() {
        this._headerItem = new PopupMenu.PopupSwitchMenuItem('Tailscale', false);
        this._headerItem.connect('toggled', () => this._service.toggleTailscale());
        this.menu.addMenuItem(this._headerItem);

        this._statusItem = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
        this._statusItem.label.add_style_class_name('tailgauge-status');
        this._statusItem.label.clutter_text.line_wrap = true;
        this.menu.addMenuItem(this._statusItem);

        this._authItem = new PopupMenu.PopupMenuItem(_('Authorize Tailscale operator'));
        this._authItem.connect('activate', () => this._service.authorizeTailscaleOperator());
        this.menu.addMenuItem(this._authItem);

        this._connectionsHeader = new PopupMenu.PopupSeparatorMenuItem(_('Connections'));
        this.menu.addMenuItem(this._connectionsHeader);
        this._connectionsSection = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this._connectionsSection);

        this._exitNodesHeader = new PopupMenu.PopupSeparatorMenuItem(_('Exit nodes'));
        this.menu.addMenuItem(this._exitNodesHeader);
        this._exitNodesSection = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this._exitNodesSection);

        this._machinesHeader = new PopupMenu.PopupSeparatorMenuItem(_('Machines'));
        this.menu.addMenuItem(this._machinesHeader);
        this._machinesSection = new PopupMenu.PopupMenuSection();
        this.menu.addMenuItem(this._machinesSection);

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        this._refreshItem = new PopupMenu.PopupMenuItem(_('Refresh'));
        this._refreshItem.connect('activate', () => this._service.refresh(true));
        this.menu.addMenuItem(this._refreshItem);

        const settingsItem = new PopupMenu.PopupMenuItem(_('Settings'));
        settingsItem.connect('activate', () => this._extension.openPreferences());
        this.menu.addMenuItem(settingsItem);
    }

    // ---- sync ------------------------------------------------------------

    _sync() {
        this._syncPanel();
        this._syncHeader();

        const signature = this._structureSignature();
        if (signature !== this._signature) {
            this._signature = signature;
            this._rebuildSections();
        } else {
            this._syncSectionOrnaments();
        }
    }

    _syncPanel() {
        const service = this._service;
        this._panelIcon.setState(!service.active && !service.needsLogin, service.needsLogin);
        this._panelIcon.opacity = service.active ? 255 : 130;

        const showName = this._settings.get_boolean('show-status-in-panel');
        const name = service.installed ? service.selfName : '';
        this._panelLabel.text = showName && name ? ` ${name}` : '';
        this._panelLabel.visible = this._panelLabel.text !== '';
    }

    _syncHeader() {
        const service = this._service;
        this._headerItem.label.text = service.installed ? (service.selfName || 'Tailscale') : 'Tailscale';
        this._headerItem.setSensitive(service.installed);
        if (this._headerItem.state !== service.active)
            this._headerItem.setToggleState(service.active);

        let status = '';
        if (!service.installed)
            status = _('Tailscale CLI is not installed or not on PATH.');
        else if (service.actionStatus !== '')
            status = service.actionStatus;
        else if (service.lastError !== '')
            status = service.lastError;
        else if (service.active)
            status = _(ACTIVE_PHRASES[this._phraseIndex % ACTIVE_PHRASES.length]);
        else
            status = _('Tailscale is disconnected');

        this._statusItem.label.text = status;
        const isError = service.installed && service.lastError !== '' && service.actionStatus === '';
        if (isError)
            this._statusItem.label.add_style_class_name('tailgauge-error');
        else
            this._statusItem.label.remove_style_class_name('tailgauge-error');

        this._authItem.visible = service.accountsAccessDenied;
        this._refreshItem.setSensitive(service.installed);
    }

    // A rebuild while the menu is open throws away hover and key focus, so it
    // only happens when the set of rows actually changed, not on every poll.
    _structureSignature() {
        const service = this._service;
        const parts = [
            service.installed ? '1' : '0',
            service.active ? '1' : '0',
            service.accountsAccessDenied ? '1' : '0',
            service.fileSharing ? '1' : '0',
            service.accounts.map(a => a.id).join(','),
            this._displayExitNodes().map(n => n.id).join(','),
            service.peers.map(p => p.id).join(','),
        ];
        return parts.join('|');
    }

    _displayExitNodes() {
        const service = this._service;
        const recent = Model.recentMullvadNodes(
            service.mullvadRegions,
            this._settings.get_strv('recent-mullvad-regions'),
            RECENT_MULLVAD_LIMIT);
        const nodes = service.tailnetExitNodes.concat(recent);
        if (service.mullvadRegions.length > 0)
            nodes.push({id: 'mullvad:add', AddMullvad: true, DisplayName: _('Choose Mullvad region')});
        return nodes;
    }

    _rebuildSections() {
        this._rebuildConnections();
        this._rebuildExitNodes();
        this._rebuildMachines();
    }

    _syncSectionOrnaments() {
        const service = this._service;
        for (const item of this._connectionsSection._getMenuItems()) {
            if (!item._accountId)
                continue;
            item.setOrnament(item._accountId === service.selectedAccountId
                ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        }
        const nodes = this._displayExitNodes();
        for (const item of this._exitNodesSection._getMenuItems()) {
            if (!item._exitNodeId)
                continue;
            const node = nodes.find(n => String(n.id) === item._exitNodeId);
            item.setOrnament(node && node.ExitNode === true
                ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        }
    }

    _rebuildConnections() {
        const service = this._service;
        this._connectionsSection.removeAll();

        const show = service.accounts.length > 1 || service.accountsAccessDenied;
        this._connectionsHeader.visible = show;
        if (!show)
            return;

        for (const account of service.accounts) {
            const item = new PopupMenu.PopupMenuItem(Model.accountLabel(account));
            item._accountId = String(account.id || '');
            item.setOrnament(account.selected === true
                ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
            item.connect('activate', () => this._service.switchAccount(account.id));
            this._connectionsSection.addMenuItem(item);
        }
    }

    _rebuildExitNodes() {
        const service = this._service;
        this._exitNodesSection.removeAll();

        const nodes = this._displayExitNodes();
        const show = service.active && (nodes.length > 0 || service.mullvadRegions.length > 0);
        this._exitNodesHeader.visible = show;
        if (!show)
            return;

        for (const node of nodes) {
            if (node.AddMullvad === true) {
                this._exitNodesSection.addMenuItem(this._buildMullvadPicker());
                continue;
            }
            const item = new PopupMenu.PopupMenuItem(
                String(node.DisplayName || node.HostName || _('Unknown')));
            item._exitNodeId = String(node.id || '');
            item.setOrnament(node.ExitNode === true
                ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
            item.connect('activate', () => this._chooseExitNode(node));
            this._exitNodesSection.addMenuItem(item);
        }
    }

    _buildMullvadPicker() {
        const submenu = new PopupMenu.PopupSubMenuMenuItem(_('Choose Mullvad region'), true);
        submenu.icon.icon_name = 'network-vpn-symbolic';

        const searchItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        const entry = new St.Entry({
            style_class: 'tailgauge-search',
            hint_text: _('Search regions'),
            can_focus: true,
            x_expand: true,
        });
        entry.clutter_text.connect('text-changed', () => {
            this._mullvadQuery = entry.get_text();
            this._fillMullvadRegions(submenu.menu);
        });
        searchItem.add_child(entry);
        submenu.menu.addMenuItem(searchItem);

        this._mullvadRegionsFrom = submenu.menu.numMenuItems;
        this._fillMullvadRegions(submenu.menu);

        submenu.menu.connect('open-state-changed', (_menu, open) => {
            if (open)
                GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                    entry.grab_key_focus();
                    return GLib.SOURCE_REMOVE;
                });
        });

        return submenu;
    }

    _fillMullvadRegions(menu) {
        for (const item of menu._getMenuItems().slice(this._mullvadRegionsFrom))
            item.destroy();

        const regions = Model.filterMullvadRegions(this._service.mullvadRegions, this._mullvadQuery);
        if (regions.length === 0) {
            const empty = new PopupMenu.PopupMenuItem(_('No Mullvad regions found.'),
                {reactive: false, can_focus: false});
            menu.addMenuItem(empty);
            return;
        }

        for (const region of regions.slice(0, 200)) {
            const subtitle = Model.mullvadRegionSubtitle(region);
            const title = Model.mullvadRegionTitle(region);
            const item = new PopupMenu.PopupMenuItem(subtitle ? `${title}, ${subtitle}` : title);
            item.setOrnament(region.ExitNode === true
                ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
            item.connect('activate', () => this._chooseExitNode(region));
            menu.addMenuItem(item);
        }
    }

    _chooseExitNode(node) {
        if (!node)
            return;
        if (node.Mullvad === true) {
            const next = Model.pushRecentMullvad(
                this._settings.get_strv('recent-mullvad-regions'),
                Model.mullvadRegionKey(node),
                RECENT_MULLVAD_LIMIT);
            this._settings.set_strv('recent-mullvad-regions', next);
        }
        this._service.setExitNode(node);
    }

    _rebuildMachines() {
        const service = this._service;
        this._machinesSection.removeAll();
        this._peerItems = [];

        const show = service.installed && service.active;
        this._machinesHeader.visible = show;
        if (!show)
            return;

        if (service.peers.length === 0) {
            this._machinesSection.addMenuItem(new PopupMenu.PopupMenuItem(
                _('No machines found on this tailnet.'), {reactive: false, can_focus: false}));
            return;
        }

        for (const peer of service.peers)
            this._machinesSection.addMenuItem(this._buildPeerItem(peer));
    }

    _buildPeerItem(peer) {
        const name = String(peer.DisplayName || peer.HostName || _('Unknown'));
        const item = new PopupMenu.PopupSubMenuMenuItem(name, true);
        item.icon.icon_name = Model.osIconName(peer.OS);
        item._peer = peer;
        this._peerItems.push(item);

        const ip = peer.TailscaleIPs && peer.TailscaleIPs.length > 0 ? String(peer.TailscaleIPs[0]) : '';
        const ipv6 = peer.TailscaleIPv6 && peer.TailscaleIPv6.length > 0 ? String(peer.TailscaleIPv6[0]) : '';
        const dns = String(peer.DNSName || '');

        const options = [];
        if (name !== '')
            options.push({label: name, run: () => this._service.copyPeerName(peer)});
        if (dns !== '')
            options.push({label: dns, run: () => this._service.copyPeerDnsName(peer)});
        if (ipv6 !== '')
            options.push({label: ipv6, run: () => this._service.copyToClipboard(ipv6)});
        if (ip !== '')
            options.push({label: ip, run: () => this._service.copyPeerIp(peer)});

        for (const option of options) {
            const copyItem = new PopupMenu.PopupMenuItem(option.label);
            copyItem.add_child(new St.Icon({
                icon_name: 'edit-copy-symbolic',
                style_class: 'popup-menu-icon tailgauge-copy-icon',
                x_expand: true,
                x_align: Clutter.ActorAlign.END,
            }));
            copyItem.connect('activate', () => {
                option.run();
                this.menu.close();
            });
            item.menu.addMenuItem(copyItem);
        }

        if (this._service.canSendFiles(peer)) {
            item.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
            const sendItem = new PopupMenu.PopupMenuItem(_('Send files…'));
            sendItem.connect('activate', () => this._sendPeerFile(peer));
            item.menu.addMenuItem(sendItem);
        }

        return item;
    }

    _sendPeerFile(peer) {
        if (!this._service.canSendFiles(peer))
            return;
        // The file chooser takes over from here, so get the menu out of the way.
        this._service.sendFile(peer);
        this.menu.close();
    }

    // ---- keyboard --------------------------------------------------------

    // PopupMenu already handles arrows and Enter; these are the single-letter
    // actions the Omarchy panel binds, resolved against whichever machine row
    // currently holds key focus.
    _onMenuKey(event) {
        const focus = global.stage.get_key_focus();
        if (focus instanceof Clutter.Text && focus.editable)
            return Clutter.EVENT_PROPAGATE;

        const symbol = event.get_key_symbol();
        const service = this._service;

        if (symbol === Clutter.KEY_t || symbol === Clutter.KEY_T) {
            service.toggleTailscale();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_r || symbol === Clutter.KEY_R) {
            service.refresh(true);
            return Clutter.EVENT_STOP;
        }

        const peer = this._focusedPeer();
        if (!peer)
            return Clutter.EVENT_PROPAGATE;

        if (symbol === Clutter.KEY_c || symbol === Clutter.KEY_C) {
            service.copyPeerIp(peer);
            this.menu.close();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_n || symbol === Clutter.KEY_N) {
            service.copyPeerName(peer);
            this.menu.close();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_d || symbol === Clutter.KEY_D) {
            service.copyPeerDnsName(peer);
            this.menu.close();
            return Clutter.EVENT_STOP;
        }
        if (symbol === Clutter.KEY_s || symbol === Clutter.KEY_S) {
            this._sendPeerFile(peer);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _focusedPeer() {
        let actor = global.stage.get_key_focus();
        while (actor) {
            if (actor._delegate && actor._delegate._peer)
                return actor._delegate._peer;
            if (actor._peer)
                return actor._peer;
            actor = actor.get_parent();
        }
        for (const item of this._peerItems) {
            if (item.active)
                return item._peer;
        }
        return null;
    }

    // ---- phrases ---------------------------------------------------------

    _startPhrases() {
        this._stopPhrases();
        this._phraseTimeoutId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PHRASE_INTERVAL_MS, () => {
            this._phraseIndex = (this._phraseIndex + 1) % ACTIVE_PHRASES.length;
            this._syncHeader();
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
