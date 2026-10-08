// Drives the TailGauge indicator inside a real shell: opens the menu, writes
// its geometry to /out/probe.txt, and screenshots it scrolled to the top and
// to the bottom. Writes /out/done when finished, which is what ends the run.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const OUT = '/out';
const lines = [];

function record(line) {
    lines.push(line);
    GLib.file_set_contents(`${OUT}/probe.txt`, `${lines.join('\n')}\n`);
}

function finish() {
    GLib.file_set_contents(`${OUT}/done`, '');
}

function later(ms, step) {
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
        try {
            step();
        } catch (e) {
            record(`error: ${e}\n${e.stack}`);
            finish();
        }
        return GLib.SOURCE_REMOVE;
    });
}

function screenshot(name, then) {
    const stream = Gio.File.new_for_path(`${OUT}/${name}.png`)
        .replace(null, false, Gio.FileCreateFlags.NONE, null);
    new Shell.Screenshot().screenshot(false, stream, (shot, result) => {
        try {
            shot.screenshot_finish(result);
        } catch (e) {
            record(`screenshot ${name}: ${e}`);
        }
        stream.close(null);
        then();
    });
}

export default class Probe extends Extension {
    enable() {
        later(6000, () => {
            const indicator = Main.panel.statusArea['tailgauge@arzaroth.github.io'];
            if (!indicator) {
                record('error: the TailGauge indicator is not in the top bar');
                finish();
                return;
            }
            indicator.menu.open(false);

            later(1500, () => {
                const monitor = Main.layoutManager.primaryMonitor;
                const popup = indicator.menu.actor;
                const scroll = indicator._scroll;
                // The section is the scrollable itself, allocated at the page
                // height on every shell, so its range is what says whether
                // every row can be reached.
                const sections = indicator._scrolled.actor;
                const [, natural] = sections.get_preferred_height(sections.width);
                const adjustment = scroll.vadjustment ?? scroll.vscroll.adjustment;
                record(`monitor: ${monitor.width}x${monitor.height}`);
                record(`popup: y=${popup.y} height=${popup.height}`);
                record(`scroll view: height=${scroll.height} ${scroll.style ?? ''}`);
                record(`sections: natural=${natural}`);
                record(`scroll: upper=${adjustment.upper} page=${adjustment.page_size}`);
                if (popup.y + popup.height > monitor.y + monitor.height)
                    record('error: the menu runs off the bottom of the monitor');
                if (natural > 0 && scroll.height <= 1)
                    record('error: the scroll view is drawn empty');
                if (adjustment.upper + 1 < natural)
                    record('error: the scroll view cannot reach the bottom of the sections');
                screenshot('top', () => {
                    adjustment.value = adjustment.upper - adjustment.page_size;
                    later(500, () => screenshot('bottom', finish));
                });
            });
        });
    }

    disable() {}
}
