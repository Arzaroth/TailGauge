// Canonical Tailscale data model, shared by the Plasma plasmoid and the GNOME
// extension. Free of module syntax so the QML engine can load it as a plain
// shared script; scripts/build.sh appends model.exports.mjs to produce the ES
// module the GNOME extension imports.

function filterIPv4(ips) {
  var result = []
  if (!ips || typeof ips.length !== "number") return result
  for (var i = 0; i < ips.length; i++) {
    var ip = String(ips[i] || "")
    if (/^100\./.test(ip)) result.push(ip)
  }
  return result
}

function filterIPv6(ips) {
  var result = []
  if (!ips || typeof ips.length !== "number") return result
  for (var i = 0; i < ips.length; i++) {
    var ip = String(ips[i] || "")
    if (/^fd7a:115c:a1e0:/i.test(ip)) result.push(ip)
  }
  return result
}

function cleanDnsName(name) {
  var value = String(name || "")
  return value.charAt(value.length - 1) === "." ? value.slice(0, -1) : value
}

function shortDnsName(name) {
  var clean = cleanDnsName(name)
  if (clean === "") return ""
  return clean.split(".")[0] || clean
}

function displayHostName(hostName, dnsName) {
  var host = String(hostName || "")
  if (host !== "" && host.toLowerCase() !== "localhost") return host
  return shortDnsName(dnsName) || host || "Unknown"
}

function isMullvadHost(name) {
  var value = String(name || "").toLowerCase()
  var suffix = ".mullvad.ts.net"
  return value.length > suffix.length && value.indexOf(suffix) === value.length - suffix.length
}

function isMullvadPeer(peer) {
  var hostName = String((peer && peer.HostName) || "")
  var dnsName = cleanDnsName((peer && peer.DNSName) || "")
  return isMullvadHost(dnsName) || isMullvadHost(hostName)
}

// Nerd Font glyphs, matching what the panel fonts on both desktops carry.
function osIcon(os) {
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
function osIconName(os) {
  var value = String(os || "").toLowerCase()
  if (value === "linux") return "computer-symbolic"
  if (value === "macos") return "computer-symbolic"
  if (value === "ios" || value === "android") return "phone-symbolic"
  if (value === "windows") return "computer-symbolic"
  if (value === "mullvad") return "network-vpn-symbolic"
  return "network-server-symbolic"
}

function accountLabel(account) {
  if (!account) return "Unknown account"
  if (account.nickname) return String(account.nickname)
  if (account.tailnet) return String(account.tailnet)
  if (account.account) return String(account.account)
  return String(account.id || "Unknown account")
}

function loginPlan(needsLogin, authUrl) {
  var url = String(authUrl || "").trim()
  if (needsLogin === true && /^https?:\/\//.test(url)) {
    return { authUrl: url, command: [] }
  }
  return { authUrl: "", command: ["tailscale", "up"] }
}

// Taildrop is a tailnet feature the admin can turn off, so the button for it
// only makes sense when this profile actually carries the capability.
function hasFileSharing(self) {
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
function isTaildropTarget(peer, selfUserId) {
  var target = peer && peer.TaildropTarget
  if (typeof target === "number" && target !== 0) return target === 1
  var owner = String((peer && peer.UserID) || "")
  return owner !== "" && owner === String(selfUserId || "")
}

function peerFromStatus(id, peer) {
  return {
    id: id,
    HostName: displayHostName(peer.HostName, peer.DNSName),
    UserID: String(peer.UserID || ""),
    TaildropTarget: typeof peer.TaildropTarget === "number" ? peer.TaildropTarget : 0,
    DNSName: cleanDnsName(peer.DNSName),
    DisplayName: displayHostName(peer.HostName, peer.DNSName),
    TailscaleIPs: filterIPv4(peer.TailscaleIPs || []),
    TailscaleIPv6: filterIPv6(peer.TailscaleIPs || []),
    Online: peer.Online === true,
    OS: String(peer.OS || ""),
    Tags: peer.Tags || [],
    ExitNodeOption: peer.ExitNodeOption === true,
    ExitNode: peer.ExitNode === true,
    Mullvad: isMullvadPeer(peer)
  }
}

function sliceTableColumn(line, start, end) {
  var text = String(line || "")
  if (start < 0 || start >= text.length) return ""
  if (end < 0) return text.substring(start).trim()
  return text.substring(start, Math.min(end, text.length)).trim()
}

function parseExitNodeList(raw) {
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
  var byHost = {}

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
      TailscaleIPs: ip ? [ip] : [],
      TailscaleIPv6: [],
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

  var result = []
  for (var hostName in byHost) result.push(byHost[hostName])
  result.sort(function(a, b) {
    var countryCompare = String(a.Country).localeCompare(String(b.Country))
    if (countryCompare !== 0) return countryCompare
    return String(a.DisplayName).localeCompare(String(b.DisplayName))
  })
  return result
}

function mullvadRegionOptions(nodes) {
  var byRegion = {}
  var values = Array.isArray(nodes) ? nodes : []
  for (var i = 0; i < values.length; i++) {
    var node = values[i] || {}
    if (node.Mullvad !== true) continue
    var country = String(node.Country || "").trim()
    var city = String(node.City || "").trim()
    if (country === "") continue
    if (city === "" || city === "Any") continue

    var key = country + "\n" + city
    if (byRegion[key]) continue

    var option = {}
    for (var propertyName in node) option[propertyName] = node[propertyName]
    option.id = "mullvad-region:" + key
    option.DisplayName = city + ", " + country
    option.Country = country
    option.City = city
    option.MullvadRegion = true
    byRegion[key] = option
  }

  var result = []
  for (var name in byRegion) result.push(byRegion[name])
  result.sort(function(a, b) {
    var countryCompare = String(a.Country).localeCompare(String(b.Country))
    if (countryCompare !== 0) return countryCompare
    return String(a.City).localeCompare(String(b.City))
  })
  return result
}

function mullvadRegionKey(node) {
  if (!node) return ""
  var country = String(node.Country || "")
  var city = String(node.City || "")
  if (country === "" || city === "") return ""
  return country + "\n" + city
}

function mullvadRegionTitle(peer) {
  if (!peer) return "Unknown"
  var city = String(peer.City || "").trim()
  var country = String(peer.Country || "").trim()
  if (city === "" || city === "Any") return country || String(peer.DisplayName || "Unknown")
  return city
}

function mullvadRegionSubtitle(peer) {
  if (!peer) return ""
  return String(peer.Country || "").trim()
}

function filterMullvadRegions(regions, query) {
  var needle = String(query || "").trim().toLowerCase()
  var values = Array.isArray(regions) ? regions : []
  var result = []
  for (var i = 0; i < values.length; i++) {
    var node = values[i]
    var label = (String(node.City || "") + " " + String(node.Country || "")).toLowerCase()
    if (needle === "" || label.indexOf(needle) !== -1) result.push(node)
  }
  return result
}

function mullvadRegionNode(regions, region) {
  var values = Array.isArray(regions) ? regions : []
  for (var i = 0; i < values.length; i++) {
    var node = values[i]
    if (mullvadRegionKey(node) === String(region || "")) return node
    if (String(node.Country || "") === String(region || "")) return node
  }
  return null
}

// The active region first, then the most recently used ones, capped at `limit`
// so the exit-node list stays a shortlist rather than the whole Mullvad fleet.
function recentMullvadNodes(regions, recent, limit) {
  var cap = typeof limit === "number" ? limit : 5
  var values = Array.isArray(regions) ? regions : []
  var history = Array.isArray(recent) ? recent : []
  var nodes = []
  var seen = {}

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

function pushRecentMullvad(recent, region, limit) {
  var cap = typeof limit === "number" ? limit : 5
  var name = String(region || "")
  if (name === "") return Array.isArray(recent) ? recent.slice(0) : []
  var history = Array.isArray(recent) ? recent : []
  var next = [name]
  for (var i = 0; i < history.length && next.length < cap; i++) {
    var existing = String(history[i] || "")
    if (existing !== "" && existing !== name && next.indexOf(existing) === -1) next.push(existing)
  }
  return next
}

function parseStatus(raw) {
  var text = String(raw || "").trim()
  if (text === "") return { ok: true, unavailable: true, message: "Disconnected" }

  try {
    var data = JSON.parse(text)
    var backendState = String(data.BackendState || "Unknown")
    var self = data.Self || {}
    // Normalized the same way as every peer, so the local machine can carry the
    // same copy options without a second shape to keep in step.
    var selfPeer = peerFromStatus("self", self)
    if (selfPeer.TailscaleIPs.length === 0 && selfPeer.TailscaleIPv6.length === 0) {
      selfPeer.TailscaleIPs = filterIPv4(data.TailscaleIPs || [])
      selfPeer.TailscaleIPv6 = filterIPv6(data.TailscaleIPs || [])
    }
    var peers = []
    var exitNodes = []
    var rawPeers = data.Peer || {}

    for (var id in rawPeers) {
      var peer = rawPeers[id] || {}
      var normalized = peerFromStatus(id, peer)
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
      backendState: backendState,
      running: backendState === "Running",
      needsLogin: backendState === "NeedsLogin",
      authUrl: String(data.AuthURL || ""),
      selfName: selfPeer.DisplayName,
      selfDnsName: selfPeer.DNSName,
      selfIp: selfPeer.TailscaleIPs.length > 0 ? selfPeer.TailscaleIPs[0] : "",
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

function parseAccounts(raw) {
  var text = String(raw || "").trim()
  if (text === "") return { accounts: [], selectedAccountId: "", selectedAccountLabel: "" }

  try {
    var parsed = JSON.parse(text)
    var next = []
    var selected = null
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

function peerAddress(peer) {
  if (!peer) return ""
  if (peer.DNSName) return cleanDnsName(peer.DNSName)
  if (peer.HostName) return String(peer.HostName)
  var ips = filterIPv4(peer.TailscaleIPs || [])
  return ips.length > 0 ? ips[0] : ""
}

function exitNodeTarget(peer) {
  if (!peer) return ""
  if (peer.Mullvad === true) {
    var mullvadIps = filterIPv4(peer.TailscaleIPs || [])
    if (mullvadIps.length > 0) return mullvadIps[0]
  }
  return peerAddress(peer)
}

function firstUrl(text, fallback) {
  var match = String(text || "").match(/https?:\/\/\S+/)
  if (match && match[0]) return match[0]
  return String(fallback || "")
}

function elideStatus(text, limit) {
  var cap = typeof limit === "number" ? limit : 140
  var value = String(text || "").replace(/\s+/g, " ").trim()
  return value.length > cap ? value.substring(0, cap - 3) + "…" : value
}

function isProfilesAccessDenied(text) {
  return /profiles access denied/i.test(String(text || ""))
}

// Plasma's executable data engine takes a command line rather than an argv, so
// every value interpolated into one has to survive the shell verbatim.
function shellQuote(value) {
  return "'" + String(value === null || value === undefined ? "" : value).replace(/'/g, "'\\''") + "'"
}

function shellCommand(argv) {
  var parts = []
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
  "Sorting tailnets",
  "Shuffling keys",
  "Watching machines"
]

function identityText(text) { return text }

function formatText(template, value) {
  return String(template).replace("%1", String(value === null || value === undefined ? "" : value))
}

function canSendFiles(state, peer) {
  if (!state || !state.fileSharing || !state.running || !peer) return false
  if (peer.Online !== true) return false
  // The KDE Store ships a kpackage and EGO ships an extension zip; neither can
  // put tailgauge-send on PATH. Without it the button would do nothing at all.
  if (state.helpers === false) return false
  return isTaildropTarget(peer, state.selfUserId)
}

function peerCopyOptions(peer) {
  if (!peer) return []
  var name = String(peer.DisplayName || peer.HostName || "")
  var dns = String(peer.DNSName || "")
  var ipv6 = peer.TailscaleIPv6 && peer.TailscaleIPv6.length > 0 ? String(peer.TailscaleIPv6[0]) : ""
  var ip = peer.TailscaleIPs && peer.TailscaleIPs.length > 0 ? String(peer.TailscaleIPs[0]) : ""
  var options = []
  if (name !== "") options.push({ kind: "name", label: name })
  if (dns !== "") options.push({ kind: "dns", label: dns })
  if (ipv6 !== "") options.push({ kind: "ipv6", label: ipv6 })
  if (ip !== "") options.push({ kind: "ip", label: ip })
  return options
}

function peerSubtitle(peer) {
  if (!peer) return ""
  var parts = []
  if (peer.TailscaleIPs && peer.TailscaleIPs.length > 0) parts.push(String(peer.TailscaleIPs[0]))
  if (peer.DNSName) parts.push(String(peer.DNSName))
  return parts.join(" · ")
}

// An offline machine keeps its copy actions - a sleeping laptop's address is
// exactly what someone needs to wake it - so the row has to say why it reads
// differently from the ones above it.
function peerRowSubtitle(peer, t) {
  var subtitle = peerSubtitle(peer)
  if (!peer || peer.Online === true) return subtitle
  return subtitle === "" ? t("Offline") : formatText(t("Offline · %1"), subtitle)
}

function panelRow(row) {
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

function panelHeader(state, t, phraseIndex) {
  var index = typeof phraseIndex === "number" ? phraseIndex : 0
  var meta = state.active
    ? t(ACTIVE_PHRASES[((index % ACTIVE_PHRASES.length) + ACTIVE_PHRASES.length) % ACTIVE_PHRASES.length])
    : t("Tailscale is disconnected")
  return {
    id: "header",
    title: state.installed ? (state.selfName || "Tailscale") : "Tailscale",
    meta: meta,
    action: "toggle",
    toggleVisible: state.installed === true,
    // Never gated on `busy`. A background status poll must not make the switch
    // unclickable, and a toggle already reports optimistically through
    // `active`, so there is nothing to protect against a second click.
    toggleEnabled: state.installed === true,
    toggleChecked: state.active === true,
    busy: state.busy === true,
    toggleHint: state.active
      ? t("Turn Tailscale off")
      : (state.needsLogin ? t("Authorize this device") : t("Turn Tailscale on")),
    crossed: !state.active && !state.needsLogin,
    warning: state.needsLogin === true,
    dimmed: !state.active
  }
}

// Precedence, in one place: a command's own progress beats a stale error, and
// both beat the idle line.
function panelStatus(state, t) {
  if (!state.installed) return { text: t("Tailscale CLI is not installed or not on PATH."), tone: "dim" }
  if (state.actionStatus) return { text: String(state.actionStatus), tone: "dim" }
  if (state.lastError) return { text: String(state.lastError), tone: "error" }
  return { text: "", tone: "" }
}

function updateSection(state, t) {
  var update = state.update || {}
  var available = update.available === true
  var rows = []

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
function selfSection(state, t) {
  var peer = state.selfPeer || null
  var copyOptions = peerCopyOptions(peer)
  var rows = []

  if (copyOptions.length > 0) {
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
    visible: state.installed === true && state.active === true && rows.length > 0,
    empty: "",
    rows: rows
  }
}

function connectionsSection(state, t) {
  var rows = []
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
    visible: accounts.length > 1 || state.accountsAccessDenied === true,
    empty: "",
    rows: rows
  }
}

function exitNodeRows(state, t, recentRegions, mullvadQuery, pickerOpen) {
  var rows = []
  var tailnet = state.tailnetExitNodes || []
  var regions = state.mullvadRegions || []
  var i

  for (i = 0; i < tailnet.length; i++) rows.push(exitNodeRow(state, tailnet[i], t))

  var recent = recentMullvadNodes(regions, recentRegions, 5)
  for (i = 0; i < recent.length; i++) rows.push(exitNodeRow(state, recent[i], t))

  if (regions.length > 0) {
    var matches = filterMullvadRegions(regions, mullvadQuery)
    var children = []
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

function exitNodeRow(state, node, t) {
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

function exitNodesSection(state, t, recentRegions, mullvadQuery, pickerOpen) {
  var rows = state.active ? exitNodeRows(state, t, recentRegions, mullvadQuery, pickerOpen) : []
  return {
    id: "exitNodes",
    title: t("Exit nodes"),
    visible: state.active === true && rows.length > 0,
    empty: "",
    rows: rows
  }
}

function machinesSection(state, t) {
  var rows = []
  var peers = state.active ? (state.peers || []) : []
  for (var i = 0; i < peers.length; i++) {
    var peer = peers[i]
    var copyOptions = peerCopyOptions(peer)
    var actions = []
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
      payload: peer
    }))
  }
  return {
    id: "machines",
    title: t("Machines"),
    visible: state.installed === true && state.active === true,
    empty: t("No machines found on this tailnet."),
    rows: rows
  }
}

// One traversal order for both desktops: the header, then every navigable row
// of every visible section, in the order they are drawn. Cursor movement is an
// index into this, so neither frontend carries a focus state machine that the
// other one could disagree with.
function panelNavigation(header, sections) {
  var nav = [{ sectionId: "header", rowId: header.id, action: header.action }]
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

function resolvePanel(state, options) {
  var opts = options || {}
  var t = typeof opts.t === "function" ? opts.t : identityText
  var source = state || {}

  var header = panelHeader(source, t, opts.phraseIndex)
  var sections = [
    updateSection(source, t),
    selfSection(source, t),
    connectionsSection(source, t),
    exitNodesSection(source, t, opts.recentRegions || [], opts.mullvadQuery || "", opts.mullvadPickerOpen === true),
    machinesSection(source, t)
  ]

  return {
    header: header,
    status: panelStatus(source, t),
    sections: sections,
    navigation: panelNavigation(header, sections)
  }
}

// Resolve a navigation entry back to the row it points at, so a frontend can
// act on the cursor without keeping its own copy of the panel.
function panelRowAt(panel, navIndex) {
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
function panelRowHasAction(row, actionId) {
  var actions = (row && row.actions) || []
  for (var i = 0; i < actions.length; i++)
    if (String(actions[i].id) === String(actionId)) return true
  return false
}

function panelNavIndexOf(panel, rowId) {
  if (!panel || !panel.navigation) return 0
  for (var i = 0; i < panel.navigation.length; i++)
    if (panel.navigation[i].rowId === String(rowId)) return i
  return 0
}
