// Canonical provider data model, shared by the Plasma plasmoid, the GNOME
// extension and the Omarchy plugin. scripts/build.sh compiles this file once
// and ships the result twice: as the ES module the GNOME extension imports,
// and with the export footer stripped as the plain shared script both QML
// engines load.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

// Everything `tailscale` hands back is parsed JSON of a shape the daemon is
// free to change between releases, so the parsers take it loosely and every
// value is coerced on the way in. Only what this model *produces* is precise.
type Raw = any

export type Translate = (text: string) => string

export interface Peer {
  id: string
  HostName: string
  UserID?: string
  UserName?: string
  TaildropTarget?: number
  DNSName: string
  DisplayName: string
  IPv4: string[]
  IPv6: string[]
  Online: boolean
  OS: string
  Tags: string[]
  ExitNodeOption: boolean
  ExitNode: boolean
  Mullvad: boolean
  Country?: string
  City?: string
  Status?: string
  MullvadRegion?: boolean
  // How the tunnel is carried, and its round trip. -1 is "not measured"
  // rather than "instant"; only NetBird reports a latency.
  ConnectionType?: string
  LatencyMs?: number
  // What the expanded row shows. Absent where the provider does not say.
  Endpoint?: string
  Relay?: string
  RxBytes?: number
  TxBytes?: number
  LastHandshake?: string
  LastSeen?: string
  Created?: string
  PublicKey?: string
  Routes?: string[]
}

// A NetBird network: a route the daemon can be told to take or leave.
export interface Network {
  id: string
  range: string
  domains: string[]
  selected: boolean
  status: string
}

export interface NetworksResult {
  ok: boolean
  networks: Network[]
  message: string
}

export interface Account {
  id: string
  nickname?: string
  tailnet?: string
  account?: string
  selected?: boolean
}

// A union rather than one bag of optionals, so a consumer that has ruled out
// the two failure shapes is handed a status whose fields are all there.
export interface StatusError {
  ok: false
  unavailable: true
  message: string
  error: string
}

export interface StatusUnavailable {
  ok: true
  unavailable: true
  message: string
}

export interface StatusOk {
  ok: true
  unavailable: false
  daemonState: string
  running: boolean
  needsLogin: boolean
  authUrl: string
  selfName: string
  selfDnsName: string
  selfIp: string
  selfUserId: string
  selfPeer: Peer
  fileSharing: boolean
  peers: Peer[]
  exitNodes: Peer[]
}

export type StatusResult = StatusOk | StatusUnavailable | StatusError

export interface AccountsResult {
  accounts: Account[]
  selectedAccountId: string
  selectedAccountLabel: string
}

export interface LoginPlan {
  authUrl: string
  command: string[]
}

export interface CopyOption {
  kind: string
  label: string
}

export interface RowAction {
  id: string
  label: string
  icon: string
  glyph: string
}

export interface UpdateInfo {
  available?: boolean
  updatable?: boolean
  latest?: string
  url?: string
  error?: string
  targets?: UpdateTarget[]
}

// One installed part, as tailgauge-update reports it. The panel reads only the
// helpers: the widget already knows its own version.
export interface UpdateTarget {
  kind?: string
  current?: string
  managed?: string
  outdated?: boolean
}

export interface PanelRow {
  id: string
  kind: string
  label: string
  sublabel: string
  icon: string
  glyph: string
  action: string
  current: boolean
  busy: boolean
  bold: boolean
  navigable: boolean
  hint: string
  actions: RowAction[]
  copyOptions: CopyOption[]
  children: PanelRow[]
  expanded: boolean
  searchPlaceholder: string
  payload: Raw
}

// What panelRow() accepts: the same shape, with everything but the identity
// optional, since each caller sets only the fields its row actually uses.
type PanelRowInput = Partial<PanelRow> & { id: string }

export interface PanelSection {
  id: string
  title: string
  visible: boolean
  empty: string
  rows: PanelRow[]
}

export interface PanelHeader {
  id: string
  title: string
  providerId: string
  icon: string
  glyph: string
  meta: string
  action: string
  toggleVisible: boolean
  toggleEnabled: boolean
  toggleChecked: boolean
  busy: boolean
  toggleHint: string
  crossed: boolean
  warning: boolean
  dimmed: boolean
}

export interface PanelStatus {
  text: string
  tone: string
}

export interface NavEntry {
  sectionId: string
  rowId: string
  action: string
}

// What one provider is doing, whether or not it is the one on screen. The bar
// icon and its tooltip are about the machine, not about the panel's current
// view, so they read these rather than the active provider's fields.
export interface ProviderSummary {
  id: string
  label: string
  running: boolean
  needsLogin: boolean
  selfName: string
  selfIp: string
  state: string
}

// The bar icon answers "am I on a mesh VPN", which is not the same question as
// "what is the panel showing".
export interface BarState {
  connected: boolean
  warning: boolean
  crossed: boolean
  tooltip: string[]
}

export interface Panel {
  bar: BarState
  header: PanelHeader
  status: PanelStatus
  sections: PanelSection[]
  footer: string
  navigation: NavEntry[]
}

// What a provider's CLI can be asked to do. The resolver reads these rather
// than the provider id, so a section is gated by the feature it needs and not
// by a name it has to know.
export interface ProviderCapabilities {
  exitNodes: boolean
  mullvad: boolean
  fileSend: boolean
  accounts: boolean
  networks: boolean
  connectionQuality: boolean
}

export type ProviderCapability = keyof ProviderCapabilities

// The argv a provider answers to. Everything past `status`, `up` and `down` is
// optional, and present only where the matching capability is.
export interface ProviderCommands {
  status: string[]
  up: string[]
  down: string[]
  exitNodeList?: string[]
  accounts?: string[]
  networks?: string[]
  // Blocks until the daemon reports a change, so the panel can ride events
  // instead of the clock. A provider without one falls back to the timer.
  watch?: (timeoutSec: number) => string[]
  switchAccount?: (accountId: string) => string[]
  setExitNode?: (target: string) => string[]
  selectNetwork?: (networkId: string, selected: boolean) => string[]
}

export interface ProviderDescriptor {
  id: string
  label: string
  // Probed on PATH to decide whether the provider is installed at all.
  cli: string
  // Whether this model can actually parse the CLI yet. A provider can be
  // installed and named by the panel before anything knows how to drive it,
  // and a frontend must not fire one provider's commands at another's binary.
  supported: boolean
  icon: string
  glyph: string
  capabilities: ProviderCapabilities
  commands: ProviderCommands
}

// One provider as a frontend found it. Frontends report every provider they
// probed, installed or not, so the resolver can tell "looked and found nothing"
// apart from "has not looked yet".
export interface ProviderState {
  id: string
  installed: boolean
}

// The frontend-owned view state the resolver reads. Every field is optional:
// a frontend that has not polled yet passes what it has.
export interface PanelState {
  providers?: ProviderState[]
  activeProviderId?: string
  summaries?: ProviderSummary[]
  installed?: boolean
  running?: boolean
  active?: boolean
  needsLogin?: boolean
  busy?: boolean
  helpers?: boolean
  updating?: boolean
  selfName?: string
  selfIp?: string
  selfUserId?: string
  selfPeer?: Peer | null
  fileSharing?: boolean
  peers?: Peer[]
  ownExitNodes?: Peer[]
  networks?: Network[]
  selectingNetworkId?: string
  mullvadRegions?: Peer[]
  accounts?: Account[]
  selectedAccountId?: string
  switchingAccountId?: string
  settingExitNodeId?: string
  accountsAccessDenied?: boolean
  actionStatus?: string
  lastError?: string
  update?: UpdateInfo
  version?: string
}

export interface ResolveOptions {
  t?: Translate
  phraseIndex?: number
  recentRegions?: string[]
  mullvadQuery?: string
  mullvadPickerOpen?: boolean
  machineQuery?: string
  expandedPeerId?: string
  nowMs?: number
}

// ---------------------------------------------------------------------------

// Every provider TailGauge knows how to drive. Order is the preference order:
// with several installed and no explicit choice, the first one wins.
var PROVIDERS: ProviderDescriptor[] = [
  {
    id: "tailscale",
    label: "Tailscale",
    cli: "tailscale",
    supported: true,
    icon: "network-vpn-symbolic",
    glyph: "\udb83\udea0",
    capabilities: {
      exitNodes: true,
      mullvad: true,
      fileSend: true,
      accounts: true,
      networks: false,
      // Empty CurAddr with a relay region is a relayed peer, so the direct or
      // relayed reading is there for the asking.
      connectionQuality: true
    },
    commands: {
      status: ["tailscale", "status", "--json"],
      up: ["tailscale", "up"],
      down: ["tailscale", "down"],
      exitNodeList: ["tailscale", "exit-node", "list"],
      watch: function (timeoutSec) {
        return ["tailgauge-watch", String(timeoutSec)]
      },
      accounts: ["tailscale", "switch", "--list", "--json"],
      switchAccount: function (accountId) {
        return ["tailscale", "switch", String(accountId || "")]
      },
      setExitNode: function (target) {
        return ["tailscale", "set", "--exit-node=" + String(target || "")]
      }
    }
  },
  {
    id: "netbird",
    label: "NetBird",
    cli: "netbird",
    supported: true,
    icon: "network-workgroup-symbolic",
    glyph: "\udb85\uddc6",
    capabilities: {
      exitNodes: false,
      mullvad: false,
      fileSend: false,
      accounts: false,
      networks: true,
      connectionQuality: true
    },
    commands: {
      status: ["netbird", "status", "--json"],
      up: ["netbird", "up"],
      down: ["netbird", "down"],
      networks: ["netbird", "networks", "list"],
      selectNetwork: function (networkId, selected) {
        return ["netbird", "networks", selected ? "select" : "deselect", String(networkId || "")]
      }
    }
  }
]

var DEFAULT_PROVIDER_ID = "tailscale"

function providerDescriptors(): ProviderDescriptor[] {
  return PROVIDERS.slice()
}

// What each frontend probes on PATH. The detection loop reads this rather than
// a list of its own, so teaching TailGauge a provider stays a one-file change.
function providerCliNames(): string[] {
  var out: string[] = []
  for (var i = 0; i < PROVIDERS.length; i++) out.push(PROVIDERS[i].cli)
  return out
}

// `which a b` prints one absolute path per binary it resolved and reports the
// misses on stderr, so the exit code is a miss count rather than an answer.
// Only stdout decides, and only lines that are real paths.
function parseProviderProbe(raw: Raw): ProviderState[] {
  var found: { [cli: string]: boolean } = {}
  var lines = String(raw || "").split(/\r?\n/)
  for (var i = 0; i < lines.length; i++) {
    var line = lines[i].trim()
    if (line.charAt(0) !== "/") continue
    var base = line.slice(line.lastIndexOf("/") + 1)
    if (base !== "") found[base] = true
  }
  var out: ProviderState[] = []
  for (var j = 0; j < PROVIDERS.length; j++) {
    out.push({ id: PROVIDERS[j].id, installed: found[PROVIDERS[j].cli] === true })
  }
  return out
}

function providerById(id: Raw): ProviderDescriptor | null {
  var wanted = String(id || "")
  for (var i = 0; i < PROVIDERS.length; i++) {
    if (PROVIDERS[i].id === wanted) return PROVIDERS[i]
  }
  return null
}

function installedProviders(state: PanelState | null | undefined): ProviderDescriptor[] {
  var source = state || {}
  var out: ProviderDescriptor[] = []
  var reported = source.providers
  if (!reported || typeof reported.length !== "number") {
    // A frontend that has not been taught to probe every provider still
    // reports the one it always drove through `installed`.
    if (source.installed === true) {
      var only = providerById(DEFAULT_PROVIDER_ID)
      if (only) out.push(only)
    }
    return out
  }
  // Registry order, not report order, so the preferred provider stays first
  // however the frontend happened to enumerate them.
  for (var i = 0; i < PROVIDERS.length; i++) {
    for (var j = 0; j < reported.length; j++) {
      var entry = reported[j]
      if (!entry || entry.installed !== true) continue
      if (String(entry.id || "") !== PROVIDERS[i].id) continue
      out.push(PROVIDERS[i])
      break
    }
  }
  return out
}

function activeProvider(state: PanelState | null | undefined): ProviderDescriptor | null {
  var available = installedProviders(state)
  if (available.length === 0) return null
  var wanted = String((state || {}).activeProviderId || "")
  for (var i = 0; i < available.length; i++) {
    if (available[i].id === wanted) return available[i]
  }
  // With no choice made, land on one this model can actually drive rather than
  // on whichever the registry lists first.
  for (var j = 0; j < available.length; j++) {
    if (available[j].supported) return available[j]
  }
  // A selection naming a provider that is gone falls back rather than blanking
  // the panel: uninstalling one provider must not strand the other.
  return available[0]
}

// Installed and parseable: the ones a switcher could actually move between.
function drivableProviders(state: PanelState | null | undefined): ProviderDescriptor[] {
  var available = installedProviders(state)
  var out: ProviderDescriptor[] = []
  for (var i = 0; i < available.length; i++) {
    if (available[i].supported) out.push(available[i])
  }
  return out
}

// Installed and parseable. The frontends poll only when this holds, so one
// provider's commands are never fired at another's binary.
function providerReady(state: PanelState | null | undefined): boolean {
  var provider = activeProvider(state)
  return provider !== null && provider.supported === true
}

function providerSupports(state: PanelState | null | undefined, capability: ProviderCapability): boolean {
  var provider = activeProvider(state)
  return provider ? provider.capabilities[capability] === true : false
}

// The name the panel calls the thing it is driving. Falls back to the product
// name so an empty panel still says what it is.
function providerCommands(state: PanelState | null | undefined): ProviderCommands | null {
  var provider = activeProvider(state)
  return provider ? provider.commands : null
}

function providerLabel(state: PanelState | null | undefined): string {
  var provider = activeProvider(state)
  return provider ? provider.label : "TailGauge"
}

// Every provider we know how to drive, for the line that tells a user with none
// of them installed what would work.
function providerLabelList(): string {
  var labels: string[] = []
  for (var i = 0; i < PROVIDERS.length; i++) labels.push(PROVIDERS[i].label)
  return labels.join(", ")
}

function filterIPv4(ips: Raw): string[] {
  var result: string[] = []
  if (!ips || typeof ips.length !== "number") return result
  for (var i = 0; i < ips.length; i++) {
    var ip = String(ips[i] || "")
    if (/^100\./.test(ip)) result.push(ip)
  }
  return result
}

function filterIPv6(ips: Raw): string[] {
  var result: string[] = []
  if (!ips || typeof ips.length !== "number") return result
  for (var i = 0; i < ips.length; i++) {
    var ip = String(ips[i] || "")
    if (/^fd7a:115c:a1e0:/i.test(ip)) result.push(ip)
  }
  return result
}

function cleanDnsName(name: Raw): string {
  var value = String(name || "")
  return value.charAt(value.length - 1) === "." ? value.slice(0, -1) : value
}

function shortDnsName(name: Raw): string {
  var clean = cleanDnsName(name)
  if (clean === "") return ""
  return clean.split(".")[0] || clean
}

function displayHostName(hostName: Raw, dnsName: Raw): string {
  var host = String(hostName || "")
  if (host !== "" && host.toLowerCase() !== "localhost") return host
  return shortDnsName(dnsName) || host || "Unknown"
}

function isMullvadHost(name: Raw): boolean {
  var value = String(name || "").toLowerCase()
  var suffix = ".mullvad.ts.net"
  return value.length > suffix.length && value.indexOf(suffix) === value.length - suffix.length
}

function isMullvadPeer(peer: Raw): boolean {
  var hostName = String((peer && peer.HostName) || "")
  var dnsName = cleanDnsName((peer && peer.DNSName) || "")
  return isMullvadHost(dnsName) || isMullvadHost(hostName)
}

// Nerd Font glyphs, matching what the panel fonts on both desktops carry.
function osIcon(os: Raw): string {
  var value = String(os || "").toLowerCase()
  if (value === "linux") return "󰌽"
  if (value === "macos" || value === "ios") return "󰀵"
  if (value === "windows") return "󰍲"
  if (value === "android") return "󰀲"
  if (value === "mullvad") return "󰖂"
  return "󰟀"
}

// Freedesktop icon names for the same set, for GNOME's symbolic icon theme
// and any Plasma fallback that would rather not depend on a Nerd Font.
function osIconName(os: Raw): string {
  var value = String(os || "").toLowerCase()
  if (value === "linux") return "computer-symbolic"
  if (value === "macos") return "computer-symbolic"
  if (value === "ios" || value === "android") return "phone-symbolic"
  if (value === "windows") return "computer-symbolic"
  if (value === "mullvad") return "network-vpn-symbolic"
  return "network-server-symbolic"
}

function accountLabel(account: Account | null | undefined): string {
  if (!account) return "Unknown account"
  if (account.nickname) return String(account.nickname)
  if (account.tailnet) return String(account.tailnet)
  if (account.account) return String(account.account)
  return String(account.id || "Unknown account")
}

function loginPlan(needsLogin: Raw, authUrl: Raw, upCommand: Raw): LoginPlan {
  var url = String(authUrl || "").trim()
  if (needsLogin === true && /^https?:\/\//.test(url)) {
    return { authUrl: url, command: [] }
  }
  var command = upCommand && typeof upCommand.length === "number"
    ? ([] as string[]).concat(upCommand) : []
  return { authUrl: "", command: command }
}

// Taildrop is a tailnet feature the admin can turn off, so the button for it
// only makes sense when this profile actually carries the capability.
function hasFileSharing(self: Raw): boolean {
  var capability = "https://tailscale.com/cap/file-sharing"
  var capMap = (self && self.CapMap) || null
  if (capMap && capMap[capability] !== undefined) return true
  var capabilities = (self && self.Capabilities) || []
  for (var i = 0; i < capabilities.length; i++) {
    if (String(capabilities[i]) === capability) return true
  }
  return false
}

// Tailscale grades every peer itself - offline, wrong owner, an OS without
// Taildrop, no peer API - so take its word when the status carries one, and
// fall back to same-owner for daemons too old to say.
function isTaildropTarget(peer: Raw, selfUserId: Raw): boolean {
  var target = peer && peer.TaildropTarget
  if (typeof target === "number" && target !== 0) return target === 1
  var owner = String((peer && peer.UserID) || "")
  return owner !== "" && owner === String(selfUserId || "")
}

// The display name first: a panel row is narrow, and "Alice Doe" fits where
// "alice.doe@example.com" would only elide.
function userLabel(user: Raw): string {
  if (!user) return ""
  var display = String(user.DisplayName || user.displayName || "")
  if (display !== "") return display
  var login = String(user.LoginName || user.loginName || "")
  if (login !== "") return login
  return String(user.ID || user.id || "")
}

function usersById(raw: Raw): { [id: string]: string } {
  var users: { [id: string]: string } = {}
  var source = raw || {}
  for (var id in source) users[String(id)] = userLabel(source[id])
  return users
}

function peerOwner(peer: Raw, users: Raw): string {
  var id = String((peer && peer.UserID) || "")
  if (id === "") return ""
  var map = users || {}
  return String(map[id] || "")
}

function peerFromStatus(id: string, peer: Raw, users: Raw): Peer {
  return {
    id: id,
    HostName: displayHostName(peer.HostName, peer.DNSName),
    UserID: String(peer.UserID || ""),
    UserName: peerOwner(peer, users),
    TaildropTarget: typeof peer.TaildropTarget === "number" ? peer.TaildropTarget : 0,
    DNSName: cleanDnsName(peer.DNSName),
    DisplayName: displayHostName(peer.HostName, peer.DNSName),
    IPv4: filterIPv4(peer.TailscaleIPs || []),
    IPv6: filterIPv6(peer.TailscaleIPs || []),
    Online: peer.Online === true,
    OS: String(peer.OS || ""),
    Tags: peer.Tags || [],
    ExitNodeOption: peer.ExitNodeOption === true,
    ExitNode: peer.ExitNode === true,
    Mullvad: isMullvadPeer(peer),
    // A direct address means the traffic is not going through a relay.
    ConnectionType: String(peer.CurAddr || "") !== "" ? "P2P" : (peer.Relay ? "Relayed" : ""),
    LatencyMs: -1,
    Endpoint: String(peer.CurAddr || ""),
    Relay: String(peer.Relay || ""),
    RxBytes: typeof peer.RxBytes === "number" ? peer.RxBytes : 0,
    TxBytes: typeof peer.TxBytes === "number" ? peer.TxBytes : 0,
    LastHandshake: isZeroTime(peer.LastHandshake) ? "" : String(peer.LastHandshake || ""),
    LastSeen: isZeroTime(peer.LastSeen) ? "" : String(peer.LastSeen || ""),
    Created: isZeroTime(peer.Created) ? "" : String(peer.Created || ""),
    PublicKey: String(peer.PublicKey || ""),
    Routes: peer.PrimaryRoutes || []
  }
}

function sliceTableColumn(line: Raw, start: number, end: number): string {
  var text = String(line || "")
  if (start < 0 || start >= text.length) return ""
  if (end < 0) return text.substring(start).trim()
  return text.substring(start, Math.min(end, text.length)).trim()
}

function parseExitNodeList(raw: Raw): Peer[] {
  var lines = String(raw || "").split(/\r?\n/)
  var header = ""
  var headerIndex = -1
  for (var i = 0; i < lines.length; i++) {
    if (/^\s*IP\s+HOSTNAME\s+COUNTRY\s+CITY\s+STATUS\s*$/.test(lines[i])) {
      header = lines[i]
      headerIndex = i
      break
    }
  }
  if (headerIndex === -1) return []

  var ipStart = header.indexOf("IP")
  var hostStart = header.indexOf("HOSTNAME")
  var countryStart = header.indexOf("COUNTRY")
  var cityStart = header.indexOf("CITY")
  var statusStart = header.indexOf("STATUS")
  var byHost: { [host: string]: Peer } = {}

  for (var j = headerIndex + 1; j < lines.length; j++) {
    var line = lines[j]
    if (/^\s*$/.test(line) || /^\s*#/.test(line)) continue

    var ip = sliceTableColumn(line, ipStart, hostStart)
    var host = sliceTableColumn(line, hostStart, countryStart)
    var country = sliceTableColumn(line, countryStart, cityStart)
    var city = sliceTableColumn(line, cityStart, statusStart)
    var status = sliceTableColumn(line, statusStart, -1)
    if (!isMullvadHost(host)) continue

    byHost[host] = {
      id: "mullvad:" + host,
      HostName: host,
      DNSName: host,
      DisplayName: (city && city !== "Any" ? city + ", " : "") + country,
      IPv4: ip ? [ip] : [],
      IPv6: [],
      Online: true,
      OS: "mullvad",
      Tags: [],
      ExitNodeOption: true,
      ExitNode: status !== "" && status !== "-",
      Mullvad: true,
      Country: country,
      City: city,
      Status: status
    }
  }

  var result: Peer[] = []
  for (var hostName in byHost) result.push(byHost[hostName])
  result.sort(function(a, b) {
    var countryCompare = String(a.Country).localeCompare(String(b.Country))
    if (countryCompare !== 0) return countryCompare
    return String(a.DisplayName).localeCompare(String(b.DisplayName))
  })
  return result
}

function mullvadRegionOptions(nodes: Raw): Peer[] {
  var byRegion: { [key: string]: Peer } = {}
  var values: Raw[] = Array.isArray(nodes) ? nodes : []
  for (var i = 0; i < values.length; i++) {
    var node = values[i] || {}
    if (node.Mullvad !== true) continue
    var country = String(node.Country || "").trim()
    var city = String(node.City || "").trim()
    if (country === "") continue
    if (city === "" || city === "Any") continue

    var key = country + "\n" + city
    if (byRegion[key]) continue

    var option: Raw = {}
    for (var propertyName in node) option[propertyName] = node[propertyName]
    option.id = "mullvad-region:" + key
    option.DisplayName = city + ", " + country
    option.Country = country
    option.City = city
    option.MullvadRegion = true
    byRegion[key] = option
  }

  var result: Peer[] = []
  for (var name in byRegion) result.push(byRegion[name])
  result.sort(function(a, b) {
    var countryCompare = String(a.Country).localeCompare(String(b.Country))
    if (countryCompare !== 0) return countryCompare
    return String(a.City).localeCompare(String(b.City))
  })
  return result
}

function mullvadRegionKey(node: Raw): string {
  if (!node) return ""
  var country = String(node.Country || "")
  var city = String(node.City || "")
  if (country === "" || city === "") return ""
  return country + "\n" + city
}

function mullvadRegionTitle(peer: Raw): string {
  if (!peer) return "Unknown"
  var city = String(peer.City || "").trim()
  var country = String(peer.Country || "").trim()
  if (city === "" || city === "Any") return country || String(peer.DisplayName || "Unknown")
  return city
}

function mullvadRegionSubtitle(peer: Raw): string {
  if (!peer) return ""
  return String(peer.Country || "").trim()
}

function filterMullvadRegions(regions: Raw, query: Raw): Peer[] {
  var needle = String(query || "").trim().toLowerCase()
  var values: Raw[] = Array.isArray(regions) ? regions : []
  var result: Peer[] = []
  for (var i = 0; i < values.length; i++) {
    var node = values[i]
    var label = (String(node.City || "") + " " + String(node.Country || "")).toLowerCase()
    if (needle === "" || label.indexOf(needle) !== -1) result.push(node)
  }
  return result
}

// Everything a machine row shows is searchable, plus the OS, so "linux" or a
// half-remembered address finds a machine as readily as its name does.
function filterMachines(peers: Raw, query: Raw): Peer[] {
  var needle = String(query || "").trim().toLowerCase()
  var values: Raw[] = Array.isArray(peers) ? peers : []
  if (needle === "") return values.slice(0)

  var result: Peer[] = []
  for (var i = 0; i < values.length; i++) {
    var peer = values[i]
    var haystack = [
      String(peer.DisplayName || ""),
      String(peer.HostName || ""),
      String(peer.DNSName || ""),
      String(peer.OS || ""),
      String(peer.UserName || ""),
      (peer.IPv4 || []).join(" "),
      (peer.IPv6 || []).join(" ")
    ].join(" ").toLowerCase()
    if (haystack.indexOf(needle) !== -1) result.push(peer)
  }
  return result
}

function mullvadRegionNode(regions: Raw, region: Raw): Peer | null {
  var values: Raw[] = Array.isArray(regions) ? regions : []
  for (var i = 0; i < values.length; i++) {
    var node = values[i]
    if (mullvadRegionKey(node) === String(region || "")) return node
    if (String(node.Country || "") === String(region || "")) return node
  }
  return null
}

// The active region first, then the most recently used ones, capped at `limit`
// so the exit-node list stays a shortlist rather than the whole Mullvad fleet.
function recentMullvadNodes(regions: Raw, recent: Raw, limit?: number): Peer[] {
  var cap = typeof limit === "number" ? limit : 5
  var values: Raw[] = Array.isArray(regions) ? regions : []
  var history: Raw[] = Array.isArray(recent) ? recent : []
  var nodes: Peer[] = []
  var seen: { [key: string]: boolean } = {}

  for (var a = 0; a < values.length && nodes.length < cap; a++) {
    var active = values[a]
    var activeKey = mullvadRegionKey(active)
    if (active.ExitNode === true && activeKey !== "" && !seen[activeKey]) {
      nodes.push(active)
      seen[activeKey] = true
    }
  }
  for (var i = 0; i < history.length && nodes.length < cap; i++) {
    var region = String(history[i] || "")
    if (region === "" || seen[region]) continue
    var node = mullvadRegionNode(values, region)
    if (node) {
      nodes.push(node)
      seen[region] = true
    }
  }
  return nodes
}

function pushRecentMullvad(recent: Raw, region: Raw, limit?: number): string[] {
  var cap = typeof limit === "number" ? limit : 5
  var name = String(region || "")
  if (name === "") return Array.isArray(recent) ? recent.slice(0) : []
  var history: Raw[] = Array.isArray(recent) ? recent : []
  var next = [name]
  for (var i = 0; i < history.length && next.length < cap; i++) {
    var existing = String(history[i] || "")
    if (existing !== "" && existing !== name && next.indexOf(existing) === -1) next.push(existing)
  }
  return next
}

function parseStatus(raw: Raw): StatusResult {
  var text = String(raw || "").trim()
  if (text === "") return { ok: true, unavailable: true, message: "Disconnected" }

  try {
    var data = JSON.parse(text)
    var daemonState = String(data.BackendState || "Unknown")
    var self = data.Self || {}
    // Normalized the same way as every peer, so the local machine can carry the
    // same copy options without a second shape to keep in step.
    var users = usersById(data.User)
    var selfPeer = peerFromStatus("self", self, users)
    if (selfPeer.IPv4.length === 0 && selfPeer.IPv6.length === 0) {
      selfPeer.IPv4 = filterIPv4(data.TailscaleIPs || [])
      selfPeer.IPv6 = filterIPv6(data.TailscaleIPs || [])
    }
    var peers: Peer[] = []
    var exitNodes: Peer[] = []
    var rawPeers = data.Peer || {}

    for (var id in rawPeers) {
      var peer = rawPeers[id] || {}
      var normalized = peerFromStatus(id, peer, users)
      if (normalized.Mullvad) continue
      peers.push(normalized)
      // An exit node that is not up is not a route anywhere.
      if (normalized.Online && normalized.ExitNodeOption) exitNodes.push(normalized)
    }

    // Online first, each half alphabetical. A machine that is up is the one
    // being looked for; one that is asleep still has an address worth copying.
    peers.sort(function(a, b) {
      if (a.Online !== b.Online) return a.Online ? -1 : 1
      return String(a.HostName).localeCompare(String(b.HostName))
    })
    exitNodes.sort(function(a, b) {
      return String(a.HostName).localeCompare(String(b.HostName))
    })

    return {
      ok: true,
      unavailable: false,
      daemonState: daemonState,
      running: daemonState === "Running",
      needsLogin: daemonState === "NeedsLogin",
      authUrl: String(data.AuthURL || ""),
      selfName: selfPeer.DisplayName,
      selfDnsName: selfPeer.DNSName,
      selfIp: selfPeer.IPv4.length > 0 ? selfPeer.IPv4[0] : "",
      selfUserId: String(self.UserID || ""),
      selfPeer: selfPeer,
      fileSharing: hasFileSharing(self),
      peers: peers,
      exitNodes: exitNodes
    }
  } catch (e) {
    return { ok: false, unavailable: true, message: "Status error", error: "Failed to parse tailscale status" }
  }
}

function parseAccounts(raw: Raw): AccountsResult {
  var text = String(raw || "").trim()
  if (text === "") return { accounts: [], selectedAccountId: "", selectedAccountLabel: "" }

  try {
    var parsed = JSON.parse(text)
    var next: Account[] = []
    var selected: Account | null = null
    if (parsed && typeof parsed.length === "number") {
      for (var i = 0; i < parsed.length; i++) {
        var rawAccount = parsed[i] || {}
        var account = {
          id: String(rawAccount.id || rawAccount.ID || ""),
          nickname: String(rawAccount.nickname || rawAccount.Nickname || rawAccount.name || rawAccount.Name || ""),
          tailnet: String(rawAccount.tailnet || rawAccount.Tailnet || ""),
          account: String(rawAccount.account || rawAccount.Account || rawAccount.loginName || rawAccount.LoginName || rawAccount.user || rawAccount.User || ""),
          selected: rawAccount.selected === true || rawAccount.Selected === true
        }
        next.push(account)
        if (account.selected === true) selected = account
      }
    }
    return {
      accounts: next,
      selectedAccountId: selected ? String(selected.id || "") : "",
      selectedAccountLabel: selected ? accountLabel(selected) : ""
    }
  } catch (e) {
    return { accounts: [], selectedAccountId: "", selectedAccountLabel: "" }
  }
}

// ---------------------------------------------------------------------------
// NetBird
// ---------------------------------------------------------------------------

function stripCidr(value: Raw): string {
  var text = String(value || "").trim()
  var slash = text.indexOf("/")
  return slash === -1 ? text : text.slice(0, slash)
}

function netbirdHostName(fqdn: Raw, fallback: Raw): string {
  var name = String(fqdn || "").trim()
  var dot = name.indexOf(".")
  if (dot > 0) return name.slice(0, dot)
  return name !== "" ? name : String(fallback || "")
}

// NetBird grades a peer with a word rather than a boolean, and only "Connected"
// is a peer you can reach right now.
function netbirdOnline(status: Raw): boolean {
  return String(status || "").toLowerCase() === "connected"
}

function isZeroTime(value: Raw): boolean {
  var text = String(value || "").trim()
  return text === "" || text.indexOf("0001-01-01") === 0
}

function netbirdPeer(raw: Raw): Peer {
  var value = raw || {}
  var fqdn = cleanDnsName(String(value.fqdn || ""))
  var ip = stripCidr(value.netbirdIp)
  var host = netbirdHostName(fqdn, ip)
  var latencyNs = Number(value.latency || 0)
  return {
    id: fqdn || ip || String(value.publicKey || ""),
    HostName: host,
    DNSName: fqdn,
    DisplayName: host,
    IPv4: ip !== "" ? [ip] : [],
    IPv6: [],
    Online: netbirdOnline(value.status),
    // NetBird's status carries no OS, so every row falls back to the generic
    // machine glyph rather than guessing one.
    OS: "",
    Tags: [],
    ExitNodeOption: false,
    ExitNode: false,
    Mullvad: false,
    Status: String(value.status || ""),
    ConnectionType: String(value.connectionType || ""),
    LatencyMs: latencyNs > 0 ? latencyNs / 1000000 : -1,
    Endpoint: String((value.iceCandidateEndpoint || {}).remote || ""),
    Relay: String(value.relayAddress || ""),
    RxBytes: Number(value.transferReceived || 0),
    TxBytes: Number(value.transferSent || 0),
    LastHandshake: isZeroTime(value.lastWireguardHandshake)
      ? "" : String(value.lastWireguardHandshake || ""),
    PublicKey: String(value.publicKey || ""),
    Routes: value.networks && typeof value.networks.length === "number" ? value.networks : []
  }
}

function splitNetworkList(value: Raw): string[] {
  var text = String(value || "").trim()
  if (text === "" || text === "-") return []
  var parts = text.split(",")
  var result: string[] = []
  for (var i = 0; i < parts.length; i++) {
    var part = parts[i].trim()
    if (part !== "") result.push(part)
  }
  return result
}

// `netbird networks list` prints a block per network rather than a table, so
// this reads fields off indented lines until the next "- ID:" starts one.
function parseNetbirdNetworks(raw: Raw): NetworksResult {
  var text = String(raw || "").trim()
  if (text === "") return { ok: true, networks: [], message: "" }
  if (text.indexOf("No networks available.") !== -1) return { ok: true, networks: [], message: "" }
  if (text.indexOf("Available Networks:") === -1) {
    return { ok: false, networks: [], message: text.split(/\r?\n/)[0] }
  }

  var lines = text.split(/\r?\n/)
  var networks: Network[] = []
  var current: Network | null = null

  for (var i = 0; i < lines.length; i++) {
    var trimmed = lines[i].trim()
    if (trimmed === "" || trimmed === "Available Networks:") continue

    var idMatch = /^-\s*ID:\s*(.*)$/.exec(trimmed)
    if (idMatch) {
      if (current) networks.push(current)
      current = { id: idMatch[1].trim(), range: "", domains: [], selected: false, status: "" }
      continue
    }
    if (!current) continue
    // A resolved-address line, which carries no field we show.
    if (/^\[.+?\]:/.test(trimmed)) continue

    var field = /^([A-Za-z ]+):\s*(.*)$/.exec(trimmed)
    if (!field) continue
    var key = field[1].trim().toLowerCase()
    var value = field[2].trim()
    // A "-" is how the CLI spells an absent value, in the range as in the
    // domain list. Neither is something to show.
    if (key === "network") current.range = value === "-" ? "" : value
    else if (key === "domains") current.domains = splitNetworkList(value)
    else if (key === "status") {
      current.status = value
      current.selected = value.toLowerCase() === "selected"
    }
  }
  if (current) networks.push(current)

  return { ok: true, networks: networks, message: "" }
}

// The one place a provider turns into its parser. A frontend polls and hands
// the bytes here rather than deciding which parser they came from.
function parseProviderStatus(state: PanelState | null | undefined, raw: Raw): StatusResult {
  var provider = activeProvider(state)
  if (provider && provider.id === "netbird") return parseNetbirdStatus(raw)
  return parseStatus(raw)
}

function networkSubtitle(network: Raw): string {
  if (!network) return ""
  if (network.range) return String(network.range)
  var domains = network.domains || []
  return domains.length > 0 ? domains.join(", ") : ""
}

function parseNetbirdStatus(raw: Raw): StatusResult {
  var text = String(raw || "").trim()
  if (text === "") return { ok: true, unavailable: true, message: "Disconnected" }

  try {
    var data = JSON.parse(text)
    if (!data || typeof data !== "object" || typeof data.length === "number") {
      return { ok: false, unavailable: true, message: "Status error", error: "Failed to parse netbird status" }
    }

    var state = String(data.daemonStatus || "")
    var lowered = state.toLowerCase()
    var running = lowered === "connected"
    var needsLogin = lowered === "needslogin" || lowered === "sessionexpired" || lowered === "loginfailed"

    var details = (data.peers && data.peers.details) || []
    var peers: Peer[] = []
    for (var i = 0; i < details.length; i++) peers.push(netbirdPeer(details[i]))
    peers.sort(function(a, b) {
      if (a.Online !== b.Online) return a.Online ? -1 : 1
      return String(a.HostName).localeCompare(String(b.HostName))
    })

    var selfFqdn = cleanDnsName(String(data.fqdn || ""))
    var selfIp = stripCidr(data.netbirdIp)
    var selfName = netbirdHostName(selfFqdn, selfIp)
    var selfPeer: Peer = {
      id: selfFqdn || selfIp || "self",
      HostName: selfName,
      DNSName: selfFqdn,
      DisplayName: selfName,
      IPv4: selfIp !== "" ? [selfIp] : [],
      IPv6: [],
      Online: running,
      OS: "",
      Tags: [],
      ExitNodeOption: false,
      ExitNode: false,
      Mullvad: false
    }

    return {
      ok: true,
      unavailable: false,
      daemonState: state,
      running: running,
      needsLogin: needsLogin,
      // NetBird prints its authorization URL on the `up` stream rather than
      // into status, so there is never one to read here.
      authUrl: "",
      selfName: selfName,
      selfDnsName: selfFqdn,
      selfIp: selfIp,
      selfUserId: "",
      selfPeer: selfPeer,
      fileSharing: false,
      peers: peers,
      exitNodes: []
    }
  } catch (e) {
    return { ok: false, unavailable: true, message: "Status error", error: "Failed to parse netbird status" }
  }
}

function peerAddress(peer: Raw): string {
  if (!peer) return ""
  if (peer.DNSName) return cleanDnsName(peer.DNSName)
  if (peer.HostName) return String(peer.HostName)
  var ips = filterIPv4(peer.IPv4 || [])
  return ips.length > 0 ? ips[0] : ""
}

function exitNodeTarget(peer: Raw): string {
  if (!peer) return ""
  if (peer.Mullvad === true) {
    var mullvadIps = filterIPv4(peer.IPv4 || [])
    if (mullvadIps.length > 0) return mullvadIps[0]
  }
  return peerAddress(peer)
}

function firstUrl(text: Raw, fallback: Raw): string {
  var match = String(text || "").match(/https?:\/\/\S+/)
  if (match && match[0]) return match[0]
  return String(fallback || "")
}

function elideStatus(text: Raw, limit?: number): string {
  var cap = typeof limit === "number" ? limit : 140
  var value = String(text || "").replace(/\s+/g, " ").trim()
  return value.length > cap ? value.substring(0, cap - 3) + "…" : value
}

function isProfilesAccessDenied(text: Raw): boolean {
  return /profiles access denied/i.test(String(text || ""))
}

// Plasma's executable data engine takes a command line rather than an argv, so
// every value interpolated into one has to survive the shell verbatim.
function shellQuote(value: Raw): string {
  return "'" + String(value === null || value === undefined ? "" : value).replace(/'/g, "'\\''") + "'"
}

function shellCommand(argv: Raw): string {
  var parts: string[] = []
  for (var i = 0; i < argv.length; i++) parts.push(shellQuote(argv[i]))
  return parts.join(" ")
}

// ---------------------------------------------------------------------------
// Panel layout
//
// The parity rule: this resolver decides WHAT is in the panel - which sections
// exist, in what order, which rows they hold, what every row says, and which
// rows the cursor can land on. A frontend decides only how a row LOOKS. If
// either desktop would otherwise have to re-derive something, it belongs here.
// ---------------------------------------------------------------------------

var ACTIVE_PHRASES = [
  "Encrypting connections",
  "Sending secrets",
  "Guarding wires",
  "Braiding packets",
  "Polishing tunnels",
  "Hiding routes",
  "Sealing ports",
  "Sorting peers",
  "Shuffling keys",
  "Watching machines"
]

function identityText(text: string): string { return text }

function formatText(template: Raw, value: Raw): string {
  return String(template).replace("%1", String(value === null || value === undefined ? "" : value))
}

function canSendFiles(state: PanelState | null | undefined, peer: Peer | null | undefined): boolean {
  if (!providerSupports(state, "fileSend")) return false
  if (!state || !state.fileSharing || !state.running || !peer) return false
  if (peer.Online !== true) return false
  // The KDE Store ships a kpackage and EGO ships an extension zip; neither can
  // put tailgauge-send on PATH. Without it the button would do nothing at all.
  if (state.helpers === false) return false
  return isTaildropTarget(peer, state.selfUserId)
}

function peerCopyOptions(peer: Raw): CopyOption[] {
  if (!peer) return []
  var name = String(peer.DisplayName || peer.HostName || "")
  var dns = String(peer.DNSName || "")
  var ipv6 = peer.IPv6 && peer.IPv6.length > 0 ? String(peer.IPv6[0]) : ""
  var ip = peer.IPv4 && peer.IPv4.length > 0 ? String(peer.IPv4[0]) : ""
  var options: CopyOption[] = []
  if (name !== "") options.push({ kind: "name", label: name })
  if (dns !== "") options.push({ kind: "dns", label: dns })
  if (ipv6 !== "") options.push({ kind: "ipv6", label: ipv6 })
  if (ip !== "") options.push({ kind: "ip", label: ip })
  var key = String(peer.PublicKey || "")
  if (key !== "") options.push({ kind: "key", label: key })
  return options
}

// The owner takes the DNS name's place rather than sitting after it: the row
// already names the machine, the full name is one click away in the copy menu,
// and a third part would only elide on Plasma and widen the menu on GNOME.
function peerSubtitle(peer: Raw): string {
  if (!peer) return ""
  var parts: string[] = []
  if (peer.IPv4 && peer.IPv4.length > 0) parts.push(String(peer.IPv4[0]))
  if (peer.UserName) parts.push(String(peer.UserName))
  else if (peer.DNSName) parts.push(String(peer.DNSName))
  return parts.join(" · ")
}

// An offline machine keeps its copy actions - a sleeping laptop's address is
// exactly what someone needs to wake it - so the row has to say why it reads
// differently from the ones above it.
function peerRowSubtitle(peer: Raw, t: Translate): string {
  var subtitle = peerSubtitle(peer)
  if (!peer || peer.Online === true) return subtitle
  return subtitle === "" ? t("Offline") : formatText(t("Offline · %1"), subtitle)
}

function panelRow(row: PanelRowInput): PanelRow {
  return {
    id: String(row.id || ""),
    kind: String(row.kind || ""),
    label: String(row.label || ""),
    sublabel: String(row.sublabel || ""),
    icon: String(row.icon || ""),
    glyph: String(row.glyph || ""),
    action: String(row.action || ""),
    current: row.current === true,
    busy: row.busy === true,
    bold: row.bold === true,
    navigable: row.navigable !== false,
    hint: String(row.hint || ""),
    actions: row.actions || [],
    copyOptions: row.copyOptions || [],
    children: row.children || [],
    expanded: row.expanded === true,
    searchPlaceholder: String(row.searchPlaceholder || ""),
    payload: row.payload === undefined ? null : row.payload
  }
}

// Build one provider's summary from whatever its status poll returned, so a
// frontend stores a shape the panel reads rather than a parser result.
function summarizeProvider(providerId: Raw, status: Raw): ProviderSummary {
  var provider = providerById(providerId)
  var summary: ProviderSummary = {
    id: provider ? provider.id : String(providerId || ""),
    label: provider ? provider.label : String(providerId || ""),
    running: false,
    needsLogin: false,
    selfName: "",
    selfIp: "",
    state: ""
  }
  if (!status || status.ok !== true || status.unavailable === true) return summary
  summary.running = status.running === true
  summary.needsLogin = status.needsLogin === true
  summary.selfName = String(status.selfName || "")
  summary.selfIp = String(status.selfIp || "")
  summary.state = String(status.daemonState || "")
  return summary
}

function summaryFor(state: PanelState, id: string): ProviderSummary | null {
  var summaries = (state || {}).summaries
  if (!summaries || typeof summaries.length !== "number") return null
  for (var i = 0; i < summaries.length; i++) {
    if (summaries[i] && String(summaries[i].id) === id) return summaries[i]
  }
  return null
}

function providerStateWord(t: Translate, summary: ProviderSummary | null): string {
  if (!summary) return t("checking\u2026")
  if (summary.needsLogin) return t("needs login")
  if (summary.running) return t("connected")
  return summary.state !== "" ? summary.state : t("disconnected")
}

// One line per installed provider, the active one first, so hovering the bar
// answers "what is up" without opening the panel.
function barTooltip(state: PanelState, t: Translate): string[] {
  var installed = installedProviders(state)
  if (installed.length === 0) {
    return [formatText(t("No supported VPN CLI on PATH. Looked for %1."), providerLabelList())]
  }
  var current = activeProvider(state)
  var ordered: ProviderDescriptor[] = []
  if (current) ordered.push(current)
  for (var i = 0; i < installed.length; i++) {
    if (!current || installed[i].id !== current.id) ordered.push(installed[i])
  }

  // The names differ in length, so the state column would start in a
  // different place on every line. Padded, it reads as a column.
  var width = 0
  for (var w = 0; w < ordered.length; w++) {
    if (ordered[w].label.length > width) width = ordered[w].label.length
  }

  var lines: string[] = []
  for (var j = 0; j < ordered.length; j++) {
    var provider = ordered[j]
    var summary = summaryFor(state, provider.id)
    var parts = [providerStateWord(t, summary)]
    if (summary && summary.running) {
      if (summary.selfName !== "") parts.push(summary.selfName)
      if (summary.selfIp !== "") parts.push(summary.selfIp)
    }
    var name = provider.label
    while (name.length < width) name += " "
    lines.push(name + "  " + parts.join(" \u00b7 "))
  }
  return lines
}

// Aggregate, deliberately: switching which provider the panel shows must not
// change an icon that describes the machine's connections.
function barState(state: PanelState, t: Translate): BarState {
  var summaries = state.summaries
  var connected = false
  var warning = false
  if (summaries && typeof summaries.length === "number" && summaries.length > 0) {
    for (var i = 0; i < summaries.length; i++) {
      if (!summaries[i]) continue
      if (summaries[i].running === true) connected = true
      if (summaries[i].needsLogin === true) warning = true
    }
  } else {
    // Nothing has reported yet: fall back to the active provider's own state.
    connected = state.active === true
    warning = state.needsLogin === true
  }
  return {
    connected: connected,
    warning: warning,
    crossed: !connected && !warning,
    tooltip: barTooltip(state, t)
  }
}

function formatBytes(value: Raw): string {
  var bytes = Number(value || 0)
  if (!(bytes > 0)) return "0 B"
  var units = ["B", "KB", "MB", "GB", "TB"]
  var i = 0
  while (bytes >= 1024 && i < units.length - 1) {
    bytes = bytes / 1024
    i += 1
  }
  // One decimal below 10 keeps "1.4 MB" from rounding to "1 MB".
  var shown = bytes >= 10 || i === 0 ? String(Math.round(bytes)) : String(Math.round(bytes * 10) / 10)
  return shown + " " + units[i]
}

function formatSince(value: Raw, nowMs: Raw): string {
  var text = String(value || "").trim()
  if (text === "") return ""
  var then = Date.parse(text)
  if (isNaN(then)) return ""
  var now = typeof nowMs === "number" ? nowMs : Date.now()
  var seconds = Math.floor((now - then) / 1000)
  if (seconds < 0) return ""
  if (seconds < 60) return "just now"
  var minutes = Math.floor(seconds / 60)
  if (minutes < 60) return minutes + (minutes === 1 ? " minute ago" : " minutes ago")
  var hours = Math.floor(minutes / 60)
  if (hours < 24) return hours + (hours === 1 ? " hour ago" : " hours ago")
  var days = Math.floor(hours / 24)
  return days + (days === 1 ? " day ago" : " days ago")
}

// How the tunnel is carried, in words rather than a provider's shorthand.
function connectionSummary(peer: Raw, t: Translate): string {
  if (!peer) return ""
  var kind = String(peer.ConnectionType || "")
  var relay = String(peer.Relay || "")
  if (kind === "P2P" || kind === "Direct") return t("Direct peer-to-peer")
  if (kind === "Relayed" || relay !== "") {
    return relay !== "" ? formatText(t("Relayed via %1"), relay) : t("Relayed")
  }
  return ""
}

// The rows behind a machine's disclosure arrow. Only what the provider
// actually reported: an absent reading is a row that is not there.
function peerDetailRows(peer: Raw, t: Translate, nowMs?: number): PanelRow[] {
  var rows: PanelRow[] = []
  if (!peer) return rows

  function detail(id: string, label: string, value: string): void {
    if (value === "") return
    rows.push(panelRow({
      id: "detail:" + id,
      kind: "detail",
      label: label,
      sublabel: value,
      navigable: false
    }))
  }

  var connection = connectionSummary(peer, t)
  var latency = typeof peer.LatencyMs === "number" && peer.LatencyMs >= 0
    ? Math.round(peer.LatencyMs) + " ms" : ""
  if (connection !== "" && latency !== "") connection = connection + " \u00b7 " + latency
  else if (connection === "") connection = latency

  detail("connection", t("Connection"), connection)
  detail("endpoint", t("Endpoint"), String(peer.Endpoint || ""))
  var handshake = formatSince(peer.LastHandshake, nowMs)
  detail("handshake", t("Last handshake"), handshake)
  // Tailscale fills the handshake and byte counters only once a session is up.
  // For an idle peer, when the control plane last saw it is all there is.
  if (handshake === "") detail("seen", t("Last seen"), formatSince(peer.LastSeen, nowMs))

  var rx = Number(peer.RxBytes || 0)
  var tx = Number(peer.TxBytes || 0)
  if (rx > 0 || tx > 0) {
    detail("transfer", t("Transfer"), "\u2193 " + formatBytes(rx) + "   \u2191 " + formatBytes(tx))
  }

  var routes = peer.Routes || []
  if (routes.length > 0) detail("routes", t("Routes"), routes.join(", "))

  // An idle peer reports nothing about a session it does not have, so the one
  // thing always known about it keeps the row from opening onto almost
  // nothing.
  if (rows.length < 3) detail("added", t("Added"), formatSince(peer.Created, nowMs))

  return rows
}

function panelHeader(state: PanelState, t: Translate, phraseIndex?: number): PanelHeader {
  var index = typeof phraseIndex === "number" ? phraseIndex : 0
  var label = providerLabel(state)
  var provider = activeProvider(state)
  // A provider we cannot drive yet is named, but its switch would do nothing.
  var present = providerReady(state)
  var meta = state.active
    ? t(ACTIVE_PHRASES[((index % ACTIVE_PHRASES.length) + ACTIVE_PHRASES.length) % ACTIVE_PHRASES.length])
    : formatText(t("%1 is disconnected"), label)
  return {
    id: "header",
    title: present ? (state.selfName || label) : label,
    providerId: provider ? provider.id : "",
    icon: provider ? provider.icon : "network-vpn-symbolic",
    glyph: provider ? provider.glyph : "\udb83\udea0",
    meta: meta,
    action: "toggle",
    toggleVisible: present,
    // Never gated on `busy`. A background status poll must not make the switch
    // unclickable, and a toggle already reports optimistically through
    // `active`, so there is nothing to protect against a second click.
    toggleEnabled: present,
    toggleChecked: state.active === true,
    busy: state.busy === true,
    toggleHint: state.active
      ? formatText(t("Turn %1 off"), label)
      : (state.needsLogin ? t("Authorize this device") : formatText(t("Turn %1 on"), label)),
    crossed: !state.active && !state.needsLogin,
    warning: state.needsLogin === true,
    dimmed: !state.active
  }
}

// Precedence, in one place: a command's own progress beats a stale error, and
// both beat the idle line.
function panelStatus(state: PanelState, t: Translate): PanelStatus {
  var provider = activeProvider(state)
  if (provider === null) {
    return {
      text: formatText(t("No supported VPN CLI on PATH. Looked for %1."), providerLabelList()),
      tone: "dim"
    }
  }
  if (!provider.supported) {
    return { text: formatText(t("%1 is installed, but TailGauge cannot drive it yet."), provider.label), tone: "dim" }
  }
  if (state.actionStatus) return { text: String(state.actionStatus), tone: "dim" }
  if (state.lastError) return { text: String(state.lastError), tone: "error" }
  return { text: "", tone: "" }
}

// The version of the widget you are looking at, in the quietest line the panel
// has. Each frontend passes its own, read from the manifest it shipped with.
function panelFooter(state: PanelState, t: Translate): string {
  var version = String(state.version || "")
  if (version === "") return ""

  var text = formatText(t("TailGauge v%1"), version)

  // The widget and the helpers install separately, so an update that only
  // half applied leaves them on different versions with nothing else on
  // screen saying which half is behind.
  var helpers = helpersVersion(state)
  if (helpers !== "" && helpers !== version)
    text += " · " + formatText(t("helpers v%1"), helpers)

  return text
}

function helpersVersion(state: PanelState): string {
  var targets = (state.update || {}).targets
  if (!targets || typeof targets.length !== "number") return ""
  for (var i = 0; i < targets.length; i++) {
    var target = targets[i]
    if (target && String(target.kind) === "helpers") return String(target.current || "")
  }
  return ""
}

function updateSection(state: PanelState, t: Translate): PanelSection {
  var update = state.update || {}
  var available = update.available === true
  var rows: PanelRow[] = []

  if (available) {
    var updatable = update.updatable === true
    rows.push(panelRow({
      id: "update",
      kind: "update",
      label: formatText(t("TailGauge %1 is available"), update.latest),
      sublabel: updatable
        ? t("Install it now")
        : t("Update it where you installed it from"),
      icon: "software-update-available-symbolic",
      glyph: "󰚰",
      action: updatable ? "update" : "openUrl",
      busy: state.updating === true,
      current: true,
      payload: update
    }))
  }

  return {
    id: "update",
    // No title: one banner does not need a section header over it.
    title: "",
    visible: available,
    empty: "",
    rows: rows
  }
}

// The local machine, rendered as a machine row: `tailscale status` already
// describes it exactly the way it describes a peer, and copying your own
// address is the one thing the header's name alone cannot do.
function providersSection(state: PanelState, t: Translate): PanelSection {
  var drivable = drivableProviders(state)
  var current = activeProvider(state)
  var rows: PanelRow[] = []
  for (var i = 0; i < drivable.length; i++) {
    var provider = drivable[i]
    var selected = current !== null && current.id === provider.id
    rows.push(panelRow({
      id: "provider:" + provider.id,
      kind: "provider",
      label: provider.label,
      icon: selected ? "checkmark-symbolic" : "network-vpn-symbolic",
      glyph: selected ? "\uf00c" : "\udb82\udd82",
      action: "switchProvider",
      current: selected,
      bold: selected,
      payload: provider
    }))
  }
  return {
    id: "providers",
    title: t("VPN"),
    // One provider is not a choice, and nought is not a list.
    visible: rows.length > 1,
    empty: "",
    rows: rows
  }
}

function selfSection(state: PanelState, t: Translate): PanelSection {
  var peer = state.selfPeer || null
  var copyOptions = peerCopyOptions(peer)
  var rows: PanelRow[] = []

  if (copyOptions.length > 0 && peer) {
    rows.push(panelRow({
      id: "self",
      kind: "self",
      label: String(peer.DisplayName || peer.HostName || t("Unknown")),
      sublabel: peerSubtitle(peer),
      icon: osIconName(peer.OS),
      glyph: osIcon(peer.OS),
      action: "copy",
      actions: [{ id: "copy", label: t("Copy"), icon: "edit-copy-symbolic", glyph: "󰆏" }],
      copyOptions: copyOptions,
      payload: peer
    }))
  }

  return {
    id: "self",
    title: t("This device"),
    visible: providerReady(state) && state.active === true && rows.length > 0,
    empty: "",
    rows: rows
  }
}

function connectionsSection(state: PanelState, t: Translate): PanelSection {
  var rows: PanelRow[] = []
  if (state.accountsAccessDenied) {
    rows.push(panelRow({
      id: "auth",
      kind: "auth",
      label: t("Authorize Tailscale operator"),
      sublabel: t("Allow this user to operate this Tailscale profile"),
      icon: "security-medium-symbolic",
      glyph: "󰒃",
      action: "authorize",
      busy: state.busy === true
    }))
  }
  var accounts = state.accounts || []
  for (var i = 0; i < accounts.length; i++) {
    var account = accounts[i]
    var id = String(account.id || "")
    var selected = account.selected === true
    rows.push(panelRow({
      id: "account:" + id,
      kind: "account",
      label: accountLabel(account),
      icon: selected ? "checkmark-symbolic" : "user-symbolic",
      glyph: selected ? "" : "",
      action: "switchAccount",
      current: selected,
      bold: selected,
      busy: String(state.switchingAccountId || "") === id,
      payload: account
    }))
  }
  return {
    id: "connections",
    title: t("Connections"),
    visible: providerSupports(state, "accounts")
      && (accounts.length > 1 || state.accountsAccessDenied === true),
    empty: "",
    rows: rows
  }
}

function exitNodeRows(state: PanelState, t: Translate, recentRegions: string[], mullvadQuery: string, pickerOpen: boolean): PanelRow[] {
  var rows: PanelRow[] = []
  var tailnet = state.ownExitNodes || []
  var regions = providerSupports(state, "mullvad") ? (state.mullvadRegions || []) : []
  var i

  for (i = 0; i < tailnet.length; i++) rows.push(exitNodeRow(state, tailnet[i], t))

  var recent = recentMullvadNodes(regions, recentRegions, 5)
  for (i = 0; i < recent.length; i++) rows.push(exitNodeRow(state, recent[i], t))

  if (regions.length > 0) {
    var matches = filterMullvadRegions(regions, mullvadQuery)
    var children: PanelRow[] = []
    if (matches.length === 0) {
      children.push(panelRow({
        id: "mullvad:empty",
        kind: "empty",
        label: t("No Mullvad regions found."),
        navigable: false
      }))
    }
    for (i = 0; i < matches.length; i++) {
      var region = matches[i]
      children.push(panelRow({
        id: "region:" + String(region.id || ""),
        kind: "mullvadRegion",
        label: mullvadRegionTitle(region),
        sublabel: mullvadRegionSubtitle(region),
        icon: "network-vpn-symbolic",
        glyph: "󰖂",
        action: "setExitNode",
        current: region.ExitNode === true,
        bold: region.ExitNode === true,
        busy: String(state.settingExitNodeId || "") === String(region.id || ""),
        hint: region.ExitNode === true ? t("Disconnect") : t("Connect"),
        payload: region
      }))
    }
    rows.push(panelRow({
      id: "mullvad:add",
      kind: "mullvadPicker",
      label: t("Choose Mullvad region"),
      icon: "list-add-symbolic",
      glyph: "+",
      action: "togglePicker",
      current: pickerOpen === true,
      expanded: pickerOpen === true,
      searchPlaceholder: t("Search regions"),
      children: children
    }))
  }
  return rows
}

function exitNodeRow(state: PanelState, node: Peer, t: Translate): PanelRow {
  var active = node.ExitNode === true
  return panelRow({
    id: "exit:" + String(node.id || ""),
    kind: "exitNode",
    label: String(node.DisplayName || node.HostName || t("Unknown")),
    icon: node.Mullvad === true ? "network-vpn-symbolic" : "network-connect-symbolic",
    glyph: node.Mullvad === true ? "󰖂" : "󱇢",
    action: "setExitNode",
    current: active,
    bold: active,
    busy: String(state.settingExitNodeId || "") === String(node.id || ""),
    hint: active ? t("Disconnect") : t("Connect"),
    payload: node
  })
}

function exitNodesSection(state: PanelState, t: Translate, recentRegions: string[], mullvadQuery: string, pickerOpen: boolean): PanelSection {
  var supported = providerSupports(state, "exitNodes")
  var rows = supported && state.active
    ? exitNodeRows(state, t, recentRegions, mullvadQuery, pickerOpen)
    : []
  return {
    id: "exitNodes",
    title: t("Exit nodes"),
    visible: supported && state.active === true && rows.length > 0,
    empty: "",
    rows: rows
  }
}

var MACHINE_SEARCH_MIN = 8

function networksSection(state: PanelState, t: Translate): PanelSection {
  var supported = providerSupports(state, "networks")
  var networks = supported ? (state.networks || []) : []
  var rows: PanelRow[] = []
  for (var i = 0; i < networks.length; i++) {
    var network = networks[i]
    var id = String(network.id || "")
    var selected = network.selected === true
    rows.push(panelRow({
      id: "network:" + id,
      kind: "network",
      label: id,
      sublabel: networkSubtitle(network),
      icon: selected ? "checkmark-symbolic" : "network-workgroup-symbolic",
      glyph: selected ? "\uf00c" : "\udb81\udedb",
      action: "selectNetwork",
      current: selected,
      bold: selected,
      busy: String(state.selectingNetworkId || "") === id,
      hint: selected ? t("Leave") : t("Join"),
      payload: network
    }))
  }
  return {
    id: "networks",
    title: t("Networks"),
    visible: supported && state.active === true && rows.length > 0,
    empty: "",
    rows: rows
  }
}

function machinesSection(state: PanelState, t: Translate, machineQuery: string,
                         expandedPeerId: string, nowMs?: number): PanelSection {
  var query = String(machineQuery || "")
  var all = state.active ? (state.peers || []) : []
  var rows: PanelRow[] = []

  // A field over three machines is clutter; over eighty it is the only way to
  // find one. Once it is on screen it stays, so it cannot disappear from under
  // whatever is being typed into it.
  if (all.length > MACHINE_SEARCH_MIN || query !== "") {
    rows.push(panelRow({
      id: "machines:search",
      kind: "machineSearch",
      searchPlaceholder: t("Search machines"),
      navigable: false
    }))
  }

  var peers = filterMachines(all, query)
  if (all.length > 0 && peers.length === 0) {
    rows.push(panelRow({
      id: "machines:empty",
      kind: "empty",
      label: t("No machines match."),
      navigable: false
    }))
  }

  for (var i = 0; i < peers.length; i++) {
    var peer = peers[i]
    var copyOptions = peerCopyOptions(peer)
    var details = peerDetailRows(peer, t, nowMs)
    var expanded = details.length > 0 && String(expandedPeerId) === String(peer.id || "")
    var actions: RowAction[] = []
    if (details.length > 0) {
      actions.push({
        id: "detail",
        label: expanded ? t("Hide details") : t("Show details"),
        icon: expanded ? "pan-up-symbolic" : "pan-down-symbolic",
        glyph: expanded ? "\udb80\udd43" : "\udb80\udd40"
      })
    }
    if (canSendFiles(state, peer))
      actions.push({ id: "send", label: t("Send files"), icon: "document-send-symbolic", glyph: "󰒊" })
    if (copyOptions.length > 0)
      actions.push({ id: "copy", label: t("Copy"), icon: "edit-copy-symbolic", glyph: "󰆏" })
    rows.push(panelRow({
      id: "peer:" + String(peer.id || ""),
      kind: "peer",
      label: String(peer.DisplayName || peer.HostName || t("Unknown")),
      sublabel: peerRowSubtitle(peer, t),
      icon: osIconName(peer.OS),
      glyph: osIcon(peer.OS),
      action: copyOptions.length > 0 ? "copy" : "",
      actions: actions,
      copyOptions: copyOptions,
      children: details,
      expanded: expanded,
      payload: peer
    }))
  }
  return {
    id: "machines",
    title: t("Machines"),
    visible: providerReady(state) && state.active === true,
    empty: t("No machines found on this tailnet."),
    rows: rows
  }
}

// One traversal order for both desktops: the header, then every navigable row
// of every visible section, in the order they are drawn. Cursor movement is an
// index into this, so neither frontend carries a focus state machine that the
// other one could disagree with.
function panelNavigation(header: PanelHeader, sections: PanelSection[]): NavEntry[] {
  var nav: NavEntry[] = [{ sectionId: "header", rowId: header.id, action: header.action }]
  for (var s = 0; s < sections.length; s++) {
    var section = sections[s]
    if (!section.visible) continue
    for (var r = 0; r < section.rows.length; r++) {
      var row = section.rows[r]
      if (!row.navigable) continue
      nav.push({ sectionId: section.id, rowId: row.id, action: row.action })
      // An expanded row's children are drawn between it and the next row, so
      // they are cursor stops in that position too. Collapsed, they are not on
      // screen and must not be.
      if (!row.expanded) continue
      for (var c = 0; c < row.children.length; c++) {
        var child = row.children[c]
        if (!child.navigable) continue
        nav.push({ sectionId: section.id, rowId: child.id, action: child.action })
      }
    }
  }
  return nav
}

function resolvePanel(state: PanelState | null | undefined, options?: ResolveOptions | null): Panel {
  var opts = options || {}
  var t = typeof opts.t === "function" ? opts.t : identityText
  var source = state || {}

  var header = panelHeader(source, t, opts.phraseIndex)
  var sections = [
    updateSection(source, t),
    providersSection(source, t),
    selfSection(source, t),
    connectionsSection(source, t),
    exitNodesSection(source, t, opts.recentRegions || [], opts.mullvadQuery || "", opts.mullvadPickerOpen === true),
    networksSection(source, t),
    machinesSection(source, t, opts.machineQuery || "",
      String(opts.expandedPeerId || ""), opts.nowMs)
  ]

  return {
    bar: barState(source, t),
    header: header,
    status: panelStatus(source, t),
    sections: sections,
    footer: panelFooter(source, t),
    navigation: panelNavigation(header, sections)
  }
}

// Resolve a navigation entry back to the row it points at, so a frontend can
// act on the cursor without keeping its own copy of the panel.
function panelRowAt(panel: Panel | null | undefined, navIndex: number): PanelRow | null {
  if (!panel || !panel.navigation || navIndex < 0 || navIndex >= panel.navigation.length) return null
  var entry = panel.navigation[navIndex]
  if (entry.sectionId === "header") return null
  for (var s = 0; s < panel.sections.length; s++) {
    var section = panel.sections[s]
    if (section.id !== entry.sectionId) continue
    for (var r = 0; r < section.rows.length; r++) {
      if (section.rows[r].id === entry.rowId) return section.rows[r]
      var children = section.rows[r].children
      for (var c = 0; c < children.length; c++)
        if (children[c].id === entry.rowId) return children[c]
    }
  }
  return null
}

// What a row's single-letter keys are allowed to do follows the actions the
// model put on it, not its kind, so a new copyable row does not have to be
// taught to two frontends' key handlers.
function panelRowHasAction(row: PanelRow | null | undefined, actionId: Raw): boolean {
  var actions = (row && row.actions) || []
  for (var i = 0; i < actions.length; i++)
    if (String(actions[i].id) === String(actionId)) return true
  return false
}

function panelNavIndexOf(panel: Panel | null | undefined, rowId: Raw): number {
  if (!panel || !panel.navigation) return 0
  for (var i = 0; i < panel.navigation.length; i++)
    if (panel.navigation[i].rowId === String(rowId)) return i
  return 0
}

export {
  providerDescriptors,
  providerCliNames,
  parseProviderProbe,
  providerReady,
  drivableProviders,
  providerCommands,
  summarizeProvider,
  barState,
  providerById,
  installedProviders,
  activeProvider,
  providerSupports,
  providerLabel,
  filterIPv4,
  filterIPv6,
  cleanDnsName,
  shortDnsName,
  displayHostName,
  isMullvadHost,
  isMullvadPeer,
  osIcon,
  osIconName,
  accountLabel,
  loginPlan,
  hasFileSharing,
  isTaildropTarget,
  userLabel,
  peerOwner,
  peerFromStatus,
  parseExitNodeList,
  mullvadRegionOptions,
  mullvadRegionKey,
  mullvadRegionTitle,
  mullvadRegionSubtitle,
  filterMullvadRegions,
  mullvadRegionNode,
  recentMullvadNodes,
  pushRecentMullvad,
  parseStatus,
  parseAccounts,
  parseNetbirdStatus,
  parseProviderStatus,
  parseNetbirdNetworks,
  networkSubtitle,
  peerAddress,
  exitNodeTarget,
  firstUrl,
  elideStatus,
  isProfilesAccessDenied,
  shellQuote,
  shellCommand,
  ACTIVE_PHRASES,
  canSendFiles,
  formatText,
  peerCopyOptions,
  peerDetailRows,
  formatBytes,
  formatSince,
  connectionSummary,
  peerSubtitle,
  filterMachines,
  resolvePanel,
  panelRowAt,
  panelRowHasAction,
  panelNavIndexOf
}
