## ADDED Requirements

### Requirement: Mobile review uses an explicit repository source
Repo City/Vibe City mobile SHALL initially support GitHub repository and pull-request snapshots. Authentication and private tokens SHALL use platform-appropriate flows/storage. Desktop local working trees, file watchers and MCP SHALL remain distinct desktop capabilities unless a later design explicitly adds a mobile equivalent.

#### Scenario: Open a GitHub pull request on a tablet
- **GIVEN** the player has access to a supported repository
- **WHEN** they select its pull request and comparison
- **THEN** the city and review views use that snapshot and retain repository/revision identity
- **AND** private authentication data is not placed in shared URLs or public game state

#### Scenario: Desktop local repository is unavailable on a phone
- **GIVEN** a repository was previously opened from a Mac's local filesystem
- **WHEN** a phone cannot access that source
- **THEN** the interface explains the unsupported source and offers a supported snapshot selection
- **AND** it does not imply that account sign-in grants access to the Mac's files

### Requirement: Touch navigation supports readable code review
Mobile review SHALL provide building approach/orbit/zoom, file search, source/diff/blame, line selection and private drafts without requiring simultaneous free-flight and fine pointer aiming. It SHALL preserve repository, revision, file and line identity across city and reading-panel transitions. Nearby geometry and source loading SHALL remain bounded and demand-driven.

#### Scenario: Inspect a changed file without a keyboard
- **GIVEN** a repository comparison is open on a supported tablet or phone
- **WHEN** the user searches for a changed file, approaches it and selects source, diff or blame
- **THEN** the chosen content is readable in the panel and refers to the same file/revision as the building

#### Scenario: Select lines and retain a private note
- **GIVEN** a file is open in the mobile reading view
- **WHEN** the user selects lines, writes a note and restarts the application
- **THEN** the draft restores against its original repository/revision/file/line identity
- **AND** nothing is published or written into the inspected repository without an explicit action

#### Scenario: Phone loses network during source loading
- **GIVEN** nearby buildings are visible but a selected file's source is not cached
- **WHEN** network access fails
- **THEN** the existing city and private drafts remain intact and the source view offers a recoverable failure
