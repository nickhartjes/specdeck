---
title: Documents and plans
description: Read the Markdown files and OpenSpec plans the agent writes.
---

<video class="video" controls preload="none" poster="/specdeck/video/plans.jpg" src="/specdeck/video/plans.mp4" aria-label="Documents and plans: the plan card in the timeline, a document tab linked to the plan's others, the task count following along, New and Changed marks, and the Markdown source (1 minute, narrated, with captions)"></video>

When the agent writes or changes a Markdown file, everyone can read the whole document, not only its diff:

- In **Files**, under **Documents**, select the file. It opens in a tab next to the terminal, so the session goes on while you read.
- When the agent works on an **OpenSpec change**, a **plan** card appears at the top of the **Timeline**. It shows the change's title and name, a button for each of its documents in the order of the change's schema, so your team's own steps (such as a feature brief) show up too, and how many tasks are done. The task count follows along live while the agent works through the tasks.
- A summary step that wrote or updated the plan offers **Read the plan** or **Read the changes**.

Only documents inside the session folder count, and not every one of them: a Markdown file git ignores, such as `notes.local.md` or a file in an ignored folder, or one with a private name, such as `credentials.md`, is not shown as a document. The timeline shows only that it changed, by its path, without a diff. To show such a document to everyone, share it as a file (see [Share files](/specdeck/guide/host/#share-files)). The plan card still shows an ignored change's documents and task count, without their text.

Each document opens in a tab of its own. A document of a plan links the plan's other documents at its top, so you can go from the design to the tasks. In **Overview**, the newest plan's first document is in front when you join.

A document tab marks the parts that changed since you last read it as **New** or **Changed**; turn **Highlight changes since you last read it** off to read without the marks. A document you open for the first time has no marks. Very large documents (over 200 KB) cannot be shown whole; the tab shows their changes instead.

A document opens **Formatted**. Switch it to **Source** to see the Markdown as written, with line numbers and highlighted syntax; the same parts are marked as new or changed there, line by line. Switching keeps your place in the document, and the next document you open is formatted again.

Plan cards come from the OpenSpec CLI on the host's computer. Without it, SpecDeck recognises OpenSpec's default layout (proposal, specs, design and tasks).
