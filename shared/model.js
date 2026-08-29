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
    var selfIps = filterIPv4(self.TailscaleIPs || data.TailscaleIPs || [])
    var peers = []
    var exitNodes = []
    var rawPeers = data.Peer || {}

    for (var id in rawPeers) {
      var peer = rawPeers[id] || {}
      var normalized = peerFromStatus(id, peer)
      if (normalized.Mullvad) continue
      if (normalized.Online) {
        peers.push(normalized)
        if (normalized.ExitNodeOption) exitNodes.push(normalized)
      }
    }

    peers.sort(function(a, b) {
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
      selfName: displayHostName(self.HostName, self.DNSName),
      selfDnsName: cleanDnsName(self.DNSName),
      selfIp: selfIps.length > 0 ? selfIps[0] : "",
      selfUserId: String(self.UserID || ""),
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
