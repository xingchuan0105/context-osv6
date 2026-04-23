# Workspace Interaction Spec

## 1. Core state model

The workspace page operates through four user-facing state domains:

- notebook shell state
- thread state
- chat state
- right rail state

The shell refactor must preserve these domains and only change presentation boundaries.

## 2. Notebook shell interactions

### 2.1 Edit notebook title

Initial state:

- notebook title is displayed as plain text button

Events:

- click title
- input text
- press `Enter`
- press `Escape`
- blur field

Expected result:

- click enters inline editing mode
- `Enter` saves
- `Escape` cancels and restores previous title
- blur saves if draft is non-empty
- empty draft should not overwrite the title

### 2.2 Create notebook

Events:

- click `New Notebook`
- enter notebook name
- confirm create

Expected result:

- creation request is sent
- success navigates to the new workspace
- failure surfaces a workspace-level error banner

### 2.3 Share and settings panels

Events:

- click `Share`
- click `Settings`
- click outside panel

Expected result:

- only one quick panel is open at a time
- outside click closes open panels
- copy actions show lightweight success feedback

## 3. Thread interactions

### 3.1 Search threads

Events:

- type in search box

Expected result:

- list filters in place by title and summary
- empty result uses explicit empty-state copy

### 3.2 Open thread

Events:

- click a thread row

Expected result:

- row becomes active
- thread messages load into center panel
- on mobile, the left drawer closes after selection

### 3.3 Create thread

Events:

- click `New Thread`

Expected result:

- create request is sent
- new thread appears in list
- created thread becomes active

### 3.4 Thread row menu

Events:

- click row menu trigger
- choose `Pin/Unpin`
- choose `Rename`
- choose `Delete`
- move pointer away or click elsewhere

Expected result:

- menu toggles anchored to the row
- pin updates ordering and badge state
- rename updates display title
- delete removes row and clears active thread if needed

## 4. Chat interactions

### 4.1 Submit message

Events:

- type question
- press send or submit form

Expected result:

- user message is appended immediately
- assistant response streams into the open thread
- composer is disabled only during submission/streaming window as required

### 4.2 Switch chat mode

Events:

- click compose `+`
- choose `RAG`, `Chat`, or `Web`

Expected result:

- menu closes after selection
- selected mode affects next submission
- RAG mode requires at least one eligible selected source

### 4.3 Message actions

User message actions:

- copy
- edit back into composer

Assistant message actions:

- copy
- add to note
- regenerate

Expected result:

- actions are secondary and low-noise
- `Add to note` appends content into the active note draft flow

### 4.4 Error handling

Events:

- SSE start failure
- stream error
- RAG submit without selected eligible sources

Expected result:

- visible error message appears in chat area
- page shell remains usable

## 5. Source interactions

### 5.1 Source list selection

Events:

- toggle one source
- use `Select all`

Expected result:

- only ready/completed sources count toward conversation scope
- selected state is visibly distinct from focused detail state

### 5.2 Open source detail

Events:

- click a source item body

Expected result:

- source detail replaces the standard source list block
- close returns to source list

### 5.3 New source

Entry points:

- `New Source` button

Modal tabs:

- file upload
- web link
- paste text

Expected result:

- each flow adds a source and closes the modal on success
- drag-and-drop and manual browse both remain supported for file upload

### 5.4 Source row menu

Events:

- open source menu
- pin
- delete
- reindex where available from detail

Expected result:

- row menu is non-blocking
- destructive actions require clear visual distinction

## 6. Notes interactions

### 6.1 Note list mode

Events:

- click `New Note`
- click note card
- click delete on note card

Expected result:

- new note opens editor mode
- existing note opens editor mode with loaded content
- delete removes the note immediately or after current runtime behavior

### 6.2 Note editor mode

Editor surface requirements:

- note title input
- content editor
- sync status
- save/export/promote/delete action area according to context

Expected result:

- edits update draft state
- autosync behavior remains intact
- back action returns to note list without leaving the workspace page

### 6.3 Promote note to source

Events:

- click `Promote to Source`

Expected result:

- note content is turned into a source document
- note promotion state is reflected in the UI

## 7. Mobile interactions

Mobile-specific rules:

- left rail is opened through top bar trigger
- right rail is opened through top bar trigger
- overlay click dismisses the drawer
- drawers must not block top-level error handling

## 8. Non-functional interaction rules

- hover states should be subtle and fast
- active states must be unmistakable
- destructive states must use danger color only where relevant
- empty states should guide the user toward the next action
- runtime loading should never collapse the shell layout
