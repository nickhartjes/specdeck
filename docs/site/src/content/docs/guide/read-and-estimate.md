---
title: Ask to read and estimate
description: Ask everyone to read something before you go on, and estimate a task together with hidden votes.
---

<video class="video" controls preload="none" poster="/specdeck/video/read-and-estimate.jpg" src="/specdeck/video/read-and-estimate.mp4" aria-label="Read and estimate: the host asks everyone to read a file with a time limit, viewers answer, then everyone votes on a task and the host keeps a value (1 minute, narrated, with captions)"></video>

As the host you can ask everyone in the session for two things: to read something before you go on, and to estimate how big a task is. Both show in a row above the panels, for you and for every viewer, and the row is gone again once they close. Nothing anyone answers is typed into the session or reaches the agent.

Both buttons are in your toolbar: **Ask to read** and **Estimate**. Viewers don't have them. A read request and a vote can be open at the same time, but only one of each.

## Ask to read

Click **Ask to read**, pick one thing and a time limit, and click **Ask to read**. You can ask to read:

- **The changes in the last prompt**: every file the agent changed since your last prompt, up to the moment you ask. Changes the agent makes after that are not added.
- **A document** the agent created or changed, such as `design.md`.
- **A code file** the agent changed. Viewers see its changes, or the whole file with the changes marked when you share [whole code files](/specdeck/guide/documents/).
- **A shared file**. In a terminal session, which has no timeline, only shared files can be read.

The time limit is none, 2, 5 or 10 minutes. Everyone sees the same countdown.

### What viewers do

A viewer sees what to read, the time left, and three buttons:

- **Open it** opens what you asked them to read in a tab.
- **I'm ready** marks them ready. Clicking it again takes it back.
- **I have a question** lets them add a short question of up to 280 characters, or none. Only you see the question's text; other viewers only see that someone has a question.

### What you see

You see each viewer as *Reading*, *Ready* or *Has a question*, the questions themselves under a disclosure, and the time left. Click **Go on** whenever you want to close the request. When the time is up it closes by itself.

The timeline then shows a card with how many were ready and how many had a question, and whether the time was up. It never names anyone or shows a question.

## Estimate

Click **Estimate** and write the task, up to 300 characters. **Fill from** offers the open tasks of the current OpenSpec plan and your last prompt; you can edit what it fills in. Pick the cards and click **Start the vote**:

| Cards | Values |
|---|---|
| Story points | 1, 2, 3, 5, 8, 13 |
| T-shirt sizes | XS, S, M, L, XL |

Both sets also have **?** (no idea) and **☕** (I need a break). The dialog starts on the set you used last.

### Voting

Everyone votes, you included, by clicking a card. Clicking your card again takes your vote back; clicking another changes it. Everyone sees who has voted, but nobody sees a value, yours included, until you click **Reveal the votes**. Until then the values stay in the session on your computer, not in anyone's window.

After the reveal everyone sees each vote by name and the spread, such as *From M to L* or *Everyone agrees: 5*. **?** and **☕** are shown but don't count in the spread. Then you can:

- **Keep** a value, which closes the vote. SpecDeck proposes the most common card, the larger on a tie; pick another card first if you want to.
- **Vote again**, which starts a new round with the same task and cards and clears every vote.
- **Stop voting**, which closes the vote and keeps nothing. You can stop at any time.

The timeline then shows a card with the task, the value you kept, the spread and the number of votes, without names.

## People who join, leave or come back

- Someone who joins while a request or vote is open gets it, with the time left, and can answer or vote.
- Someone who reconnects gets their own answer or vote back. While they are away they are not shown, and they keep their answer for when they come back.
- If you close the window while the session keeps running, everything stays open, answers still come in and the time still runs. You see where things are when you open the window again.
- Viewers on an older SpecDeck keep watching but get neither. You see them as *On an older app*, and they don't count as not ready or not voted.
- A viewer can change their answer or vote 5 times at once and then once a second. Past that, SpecDeck asks them to wait a moment.

## Notifications

A viewer whose window is in the background gets a notification when you ask to read, naming you and what to read, and when you start a vote or a new round. A request that was already open when they joined doesn't notify them. Each has its own switch under [While you watch a session](/specdeck/guide/settings/#while-you-watch-a-session), both on at first.

While your own window is in the background, you hear when a viewer has a question (with its text), when everyone has read, when the time runs out while someone is still reading, and when every viewer has voted. Each has its own switch under [While you host a session](/specdeck/guide/settings/#while-you-host-a-session), all on at first.
