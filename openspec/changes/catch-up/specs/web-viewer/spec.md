## ADDED Requirements

### Requirement: Catching up in the browser
A browser viewer who joins late SHALL see earlier prompts folded and the overview of what happened before they joined, as in the app. Reloading the page SHALL count as joining again.

#### Scenario: Late join in a browser
- **WHEN** a viewer joins in a browser after five prompts finished
- **THEN** they see the overview and the five prompts folded, as in the app

#### Scenario: Page reloaded
- **WHEN** a browser viewer who joined at the start reloads the page after three more prompts finished
- **THEN** those three prompts are folded and the overview covers them
