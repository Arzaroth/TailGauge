// The panel `tailgauge panel --json` returns, as this side reads it.
//
// The binary decides what the panel contains; these are the shapes it arrives
// in. Written out by hand rather than shared, because the other side of this
// contract is a Rust struct and there is nothing to generate them from - which
// is also why the fields are named exactly as the JSON names them.

export interface CopyOption {
    kind: string;
    label: string;
}

export interface RowAction {
    id: string;
    label: string;
    icon: string;
    glyph: string;
}

export interface PanelRow {
    id: string;
    kind: string;
    label: string;
    sublabel: string;
    icon: string;
    glyph: string;
    action: string;
    current: boolean;
    busy: boolean;
    bold: boolean;
    navigable: boolean;
    hint: string;
    actions: RowAction[];
    copyOptions: CopyOption[];
    children: PanelRow[];
    expanded: boolean;
    searchPlaceholder: string;
    /// A row with a scope is filtered live by that search field's query: it is
    /// drawn when the query is a substring of `searchKey`. The one row per
    /// scope with an empty key is the message drawn when nothing matched.
    searchScope: string;
    searchKey: string;
    payload: Payload | null;
}

export interface PanelSection {
    id: string;
    title: string;
    visible: boolean;
    empty: string;
    rows: PanelRow[];
}

export interface PanelHeader {
    id: string;
    title: string;
    providerId: string;
    icon: string;
    glyph: string;
    meta: string;
    action: string;
    toggleVisible: boolean;
    toggleEnabled: boolean;
    toggleChecked: boolean;
    busy: boolean;
    toggleHint: string;
    crossed: boolean;
    warning: boolean;
    dimmed: boolean;
    actions: RowAction[];
}

export interface BarState {
    connected: boolean;
    warning: boolean;
    crossed: boolean;
    tooltip: string[];
}

export interface NavEntry {
    sectionId: string;
    rowId: string;
    action: string;
    searchScope: string;
    searchKey: string;
}

export interface Panel {
    bar: BarState;
    header: PanelHeader;
    status: {text: string; tone: string};
    sections: PanelSection[];
    footer: string;
    navigation: NavEntry[];
}

/// What a row carries: a peer, an account, a network, a provider or the update
/// status, depending on the row. One shape rather than a union, because every
/// row is one type and the dispatcher branches on the row's `action` - and
/// because only the fields this side reads need declaring. The rest travel
/// through untouched: a payload goes back to the binary as it came.
export interface Payload {
    id?: string;
    HostName?: string;
    DisplayName?: string;
    DNSName?: string;
    IPv4?: string[];
    Mullvad?: boolean;
    selected?: boolean;
    label?: string;
    url?: string;
}

/// What only this side knows, handed to the binary on every call.
export interface Ui {
    activeProviderId?: string;
    active?: boolean;
    busy?: boolean;
    updating?: boolean;
    version?: string;
    actionStatus?: string;
    lastError?: string;
    switchingAccountId?: string;
    settingExitNodeId?: string;
    selectingNetworkId?: string;
    phraseIndex?: number;
    recentRegions?: string[];
    mullvadPickerOpen?: boolean;
    expandedPeerId?: string;
}

/// An empty panel, for the moment before the first answer arrives.
export function emptyPanel(): Panel {
    return {
        bar: {connected: false, warning: false, crossed: true, tooltip: []},
        header: {
            id: 'header', title: 'TailGauge', providerId: '', icon: '', glyph: '',
            meta: '', action: 'toggle', toggleVisible: false, toggleEnabled: false,
            toggleChecked: false, busy: false, toggleHint: '', crossed: true,
            warning: false, dimmed: true, actions: [],
        },
        status: {text: 'Checking…', tone: 'dim'},
        sections: [],
        footer: '',
        navigation: [],
    };
}

/// Whether a row is drawn at all, children included. A search field is the
/// binary's decision, so a query left behind when one goes would filter a list
/// with nothing on screen left to clear it.
export function panelHasRow(panel: Panel, rowId: string): boolean {
    return panel.sections.some(section => section.rows.some(
        row => row.id === rowId || row.children.some(child => child.id === rowId)));
}

/// The pair that names a Mullvad region is already in the id the binary minted
/// for it, so this reads that rather than rebuilding it from the peer.
export function mullvadRegionKey(peer: Payload | null | undefined): string {
    const id = String(peer?.id ?? '');
    const prefix = 'mullvad-region:';
    return id.startsWith(prefix) ? id.slice(prefix.length) : '';
}

/// The chosen region to the front, the rest in the order they were, capped.
export function pushRecentMullvad(recent: string[], region: string, limit: number): string[] {
    if (region === '')
        return recent.slice(0);
    const next = [region];
    for (const existing of recent) {
        if (next.length >= limit)
            break;
        if (existing !== '' && existing !== region && !next.includes(existing))
            next.push(existing);
    }
    return next;
}
