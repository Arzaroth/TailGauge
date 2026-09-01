import Adw from 'gi://Adw';
import Gdk from 'gi://Gdk';
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

        const shortcutGroup = new Adw.PreferencesGroup({
            title: _('Shortcut'),
            description: _('A key that turns Tailscale on and off from anywhere.'),
        });
        page.add(shortcutGroup);

        const shortcutRow = new Adw.ActionRow({
            title: _('Toggle Tailscale'),
            activatable: true,
        });
        const shortcutLabel = new Gtk.ShortcutLabel({
            disabled_text: _('Disabled'),
            valign: Gtk.Align.CENTER,
        });
        const clearShortcut = new Gtk.Button({
            icon_name: 'edit-clear-symbolic',
            tooltip_text: _('Clear'),
            valign: Gtk.Align.CENTER,
            css_classes: ['flat'],
        });
        clearShortcut.connect('clicked', () => settings.set_strv('toggle-shortcut', []));
        shortcutRow.add_suffix(shortcutLabel);
        shortcutRow.add_suffix(clearShortcut);
        shortcutRow.connect('activated', () => captureShortcut(window, settings));
        shortcutGroup.add(shortcutRow);

        const syncShortcut = () => {
            shortcutLabel.accelerator = settings.get_strv('toggle-shortcut')[0] ?? '';
        };
        syncShortcut();
        const shortcutChangedId = settings.connect('changed::toggle-shortcut', syncShortcut);

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
        window.connect('close-request', () => {
            settings.disconnect(recentChangedId);
            settings.disconnect(shortcutChangedId);
        });
    }
}

// A modifier is required: a global binding on a bare letter would swallow that
// key everywhere, including inside whatever you are typing in.
function captureShortcut(window, settings) {
    const dialog = new Gtk.Window({
        transient_for: window,
        modal: true,
        resizable: false,
        title: _('Toggle Tailscale'),
    });

    const box = new Gtk.Box({
        orientation: Gtk.Orientation.VERTICAL,
        spacing: 12,
        margin_top: 24,
        margin_bottom: 24,
        margin_start: 24,
        margin_end: 24,
    });
    box.append(new Gtk.Label({label: _('Press the shortcut you want, with a modifier.')}));
    box.append(new Gtk.Label({
        label: _('Backspace clears it, Escape cancels.'),
        css_classes: ['dim-label'],
    }));
    dialog.set_child(box);

    const keys = new Gtk.EventControllerKey();
    keys.connect('key-pressed', (_controller, keyval, keycode, state) => {
        const mask = state & Gtk.accelerator_get_default_mod_mask() & ~Gdk.ModifierType.LOCK_MASK;

        if (mask === 0 && keyval === Gdk.KEY_Escape) {
            dialog.close();
            return true;
        }
        if (mask === 0 && keyval === Gdk.KEY_BackSpace) {
            settings.set_strv('toggle-shortcut', []);
            dialog.close();
            return true;
        }
        if (mask === 0 || !Gtk.accelerator_valid(keyval, mask))
            return true;

        settings.set_strv('toggle-shortcut',
            [Gtk.accelerator_name_with_keycode(null, keyval, keycode, mask)]);
        dialog.close();
        return true;
    });
    dialog.add_controller(keys);
    dialog.present();
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
