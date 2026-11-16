---
title: The sharing service
description: Invite codes, what the sharing service and the relay see and keep, and their limits.
---

The sharing service is two programs: the **server**, which matches viewers to hosts and passes on the messages that set up their connection, and the **TURN relay**, which forwards encrypted traffic when host and viewer cannot reach each other directly. Neither one handles the session's content in a form it can read. [How the sharing service works](/specdeck/self-host/overview/) describes them in more detail.

## The invite code

A session's invite code, such as `7K2M-9QXH-4TPA`, is 12 random characters from your computer's secure random source: 60 bits, far too many to guess by trying them on the service.

- **It is the only key.** Anyone with the code can watch, up to the session's viewer limit (20 unless the host lowers it). There is no approval step: the host sees each viewer by the name they chose, and can remove them.
- **It works until the host replaces it or ends the session.** It does not expire on its own. **New code** in the session window replaces it at once, and removing a viewer also makes a new code, so they cannot come back with the old one. Viewers already watching stay connected; a viewer who loses the connection rejoins with a separate random token, not the code, but only while the code they left under is still the current one. A new code also ends joins that are still in progress with the old one.
- **It is never sent to the service.** The app turns the code into two values: a **room id**, which the service uses to find the session, and a **key** that stays in the apps. Host and viewer sign the messages that set up their connection with that key. Those messages also carry the fingerprint of each side's encryption certificate, so a service that swaps in its own connection details is refused, and no session content is shown. In a web join link, the code sits after the `#`, which browsers never send to a server.
- **Guessing on the service is limited.** It refuses further joins from a network after 10 wrong codes in a minute, and takes at most 60 joins per minute for any one session.

The host's app also signs what it sends on the encrypted connection. That does not protect against the service any further: the key for checking those signatures arrives over the same encrypted connection.

### The weakness: guessing a code offline

The room id the service sees is made from the code with a fast, fixed calculation. So someone who runs the sharing service, or who can read its traffic, can take a room id and test codes against it on their own computers, without asking the service. The limits above do not apply to that, and every guess can be checked against all open rooms at once. 60 bits is a large search, but with a fast calculation it is within reach of someone with serious computing power, not of a casual attacker.

What finding a code would give them:

- While the code is still valid, they can put themselves between the host and **new** viewers who join through that service, and read what those viewers receive, which is the whole session so far.
- They **cannot** read the traffic of viewers who were already connected, or recordings of earlier traffic: those connections use keys made fresh for every connection.

What you can do today:

- **Run your own sharing service**, so the people who could try this are your own team. See [Run your own](#run-your-own).
- **Make a new code once everyone has joined**, with **New code**. A code that no longer works is no use to anyone who finds it.
- **End sessions when you are done**, rather than leaving them open.

A fix is planned: the apps will agree on a key with a password-authenticated key exchange (PAKE), so a code can no longer be checked without the host, and each guess costs a real join attempt. It is not built yet, and there is no date.

## What the service sees and keeps

**The server** sees:

- the IP addresses of hosts and viewers;
- room ids, and when sessions start and end;
- how many viewers join each session, and when;
- the messages that set up each connection, which list the network addresses each app can be reached at. From the app these include addresses in your local network; browsers usually hide those.

It never sees names, the invite code, or anything of the session itself. It keeps rooms and connections in memory only, and loses them when it restarts.

Its log records what it does, one line per event: when rooms open and close and how many viewers joined each, when viewers join and leave, joins it turned away and why (such as a wrong code), connections that failed, which of its limits were reached, and every 5 minutes how many connections, rooms and viewers are open. Rooms and viewers appear only as numbers it counts up from 1 each time it starts. So the log shows when sessions happen and how many people watch them, but not which session or who.

The log never holds room ids, invite codes, connection ids, the messages that set up a connection, relay credentials or IP addresses, at any level, and there is no setting to add them. How long the built-in service keeps its log is not stated yet. See [Logs](/specdeck/self-host/configuration/#logs) for every event.

**The relay** forwards the encrypted traffic of relayed connections and cannot read it. It sees the IP addresses of both ends and how much traffic passes. It writes a log line for each connection it relays, with the addresses of both ends. How long the built-in service keeps those logs is not stated yet.

**On the built-in service**, the server and the web viewer are reached through a Cloudflare Tunnel, so Cloudflare sees what the server sees, and delivers the web viewer's code. See [Who runs the built-in sharing service](/specdeck/security/overview/#who-runs-the-built-in-sharing-service).

## The relay and its limits

The relay forwards traffic only for apps that host a session or have joined one: the server hands out relay credentials only to those, one per connection, valid for 12 hours. The relay then:

- **never forwards to private addresses**: loopback, private, link-local and other special-purpose addresses, for IPv4 and IPv6, so it cannot be used to reach the network it runs in;
- **limits each credential and the relay as a whole** to a number of relayed connections, and each connection to about 20 Mbit/s, so one session cannot use it all.

The server bounds everything it holds as well: connections in total and per network, rooms, viewers per host, and how fast set-up messages may flow. When a limit is reached, the apps say **"The sharing service is busy"** and keep retrying where they already would. Too many wrong codes from one network show **"Too many attempts"**. Self-hosters can change each limit; see [Limits](/specdeck/self-host/configuration/#limits).

## The web viewer's code

People who watch in a browser load the web viewer's code from the sharing service each time. Whoever runs the service could change that code and read the sessions of everyone who watches in a browser from then on. Viewers who use the app do not depend on it: the app is installed once, and checks every update against SpecDeck's signature. On the built-in service, that trust covers its maintainer and Cloudflare. See [The web viewer's code](/specdeck/self-host/overview/#the-web-viewers-code) for how to check the published server image.

## Run your own

Running your own sharing service puts all of this in your team's hands:

- who sees IP addresses and when sessions happen, how much the server logs, and how long logs are kept;
- the web viewer's code, built from the repository or from the signed image;
- who could attempt the offline guessing above;
- uptime and limits.

Session content does not change: it never passed through the service. Setup takes one server with a public IPv4 address and two DNS names. Start with [How the sharing service works](/specdeck/self-host/overview/), then [Docker Compose](/specdeck/self-host/docker-compose/) or [Kubernetes](/specdeck/self-host/kubernetes/), and [connect the apps](/specdeck/self-host/connect-apps/).
