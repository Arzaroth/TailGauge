import Adw from 'gi://Adw';
import Gio from 'gi://Gio';
import Gtk from 'gi://Gtk';

import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

export default class TailGaugePreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();

        const page = new Adw.PreferencesPage({
            title: _('General'),
            icon_name: 'network-vpn-symbolic',
        });
        window.add(page);

        const panelGroup = new Adw.PreferencesGroup({
            title: _('Panel'),
            description: _('How the indicator looks in the top bar.'),
        });
        page.add(panelGroup);

        const showStatus = new Adw.SwitchRow({
            title: _('Show the machine name'),
            subtitle: _('When off, the panel shows only the Tailscale icon.'),
        });
        panelGroup.add(showStatus);
        settings.bind('show-status-in-panel', showStatus, 'active', Gio.SettingsBindFlags.DEFAULT);

        const pollGroup = new Adw.PreferencesGroup({
            title: _('Polling'),
            description: _('How often `tailscale status` is re-read.'),
        });
        page.add(pollGroup);

        const interval = new Adw.SpinRow({
            title: _('Refresh interval'),
            subtitle: _('Seconds'),
            adjustment: new Gtk.Adjustment({
                lower: 5,
                upper: 3600,
                step_increment: 5,
                page_increment: 30,
            }),
        });
        pollGroup.add(interval);
        settings.bind('refresh-interval', interval, 'value', Gio.SettingsBindFlags.DEFAULT);

        const mullvadGroup = new Adw.PreferencesGroup({
            title: _('Mullvad'),
            description: _('The exit node list keeps a shortlist of the regions you use.'),
        });
        page.add(mullvadGroup);

        const recent = new Adw.ActionRow({
            title: _('Recent regions'),
            subtitle: describeRecent(settings.get_strv('recent-mullvad-regions')),
        });
        const clear = new Gtk.Button({
            label: _('Clear'),
            valign: Gtk.Align.CENTER,
        });
        clear.connect('clicked', () => settings.set_strv('recent-mullvad-regions', []));
        recent.add_suffix(clear);
        recent.activatable_widget = clear;
        mullvadGroup.add(recent);

        const recentChangedId = settings.connect('changed::recent-mullvad-regions', () => {
            recent.subtitle = describeRecent(settings.get_strv('recent-mullvad-regions'));
        });
        window.connect('close-request', () => settings.disconnect(recentChangedId));
    }
}

// Regions are stored as "Country\nCity" so the key survives a rename of either
// half; nothing else should ever see that spelling.
function describeRecent(regions) {
    if (!regions || regions.length === 0)
        return _('None yet');
    return regions
        .map(region => {
            const [country, city] = String(region).split('\n');
            return city ? `${city}, ${country}` : country;
        })
        .join(' · ');
}
