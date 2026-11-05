---
title: Troubleshooting
description: What SpecDeck's messages mean and what to do about them.
---

## Joining does not work

SpecDeck says why on the start screen:

| Message | What it means |
|---|---|
| That code does not work | Check the code with the host. Codes stop working when the host ends the session or makes a new code |
| This session is full | The host reached their viewer limit |
| Too many attempts | Several wrong codes were tried from your network. Wait a minute |
| SpecDeck cannot reach its sharing service | Check your internet connection, and the [sharing service in Settings](/specdeck/guide/settings/#sharing-service) |
| Your network does not allow the connection | Your network blocks UDP. Try another network, such as your phone's hotspot |
| Update SpecDeck to join / The host needs to update | You and the host use versions that do not match. Versions that offer Codex, Grok and pi as agents cannot connect to older versions, in either direction |
| This web viewer needs to be updated | In the [web viewer](/specdeck/guide/join/#watch-in-a-browser): the host uses a newer SpecDeck than the page the sharing service serves. Ask whoever runs the service to update it, or watch in the app |
| This browser blocks the connection | In the [web viewer](/specdeck/guide/join/#watch-in-a-browser): the browser has WebRTC turned off, as LibreWolf can. Allow WebRTC (`media.peerconnection.enabled` in `about:config` for LibreWolf and Firefox), use another browser, or watch in the app |

A code typed on its own always uses your own sharing service. If the host uses another one, the code does not work for you: ask for the link instead (see [Through another sharing service](/specdeck/guide/join/#through-another-sharing-service)).

## Clicking a link does nothing

SpecDeck 1.0 cannot open links from newer versions. Paste the link into **Join a session**, or [update](/specdeck/install/#update). Pasting a link always works, also when your system does not open `specdeck://` links.

## Sharing is unavailable

When the host sees a banner that sharing is unavailable, SpecDeck cannot reach its sharing service. The session keeps running on the host's computer, and the invite code appears as soon as the service is back. If it does not come back, check the address in [Settings](/specdeck/guide/settings/#sharing-service).

## macOS: viewers on the same network cannot connect

SpecDeck connects directly to teammates on the same network when it can, which needs macOS's Local Network permission. If you chose **Don't Allow** when macOS asked, turn SpecDeck on under **System Settings › Privacy & Security › Local Network**.

## The app does not open the first time

Releases are not signed yet. See [The first launch](/specdeck/install/#the-first-launch).
