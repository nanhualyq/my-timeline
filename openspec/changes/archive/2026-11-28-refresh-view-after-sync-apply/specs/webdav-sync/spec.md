## MODIFIED Requirements

### Requirement: The reader can see the sync status

The Settings dialog SHALL show when the last sync ran and what it did — up to and including "nothing changed" — and the reason it failed when it failed. The status SHALL be recorded so that a sync that ran before the dialog was opened is still reported.

The status SHALL state, separately from the outcome, whether that sync applied changes to the local library. The outcome SHALL keep describing what happened to the remote document (`synced`, `created`, `nothing_changed`, `failed`); a sync that downloaded and merged remote changes without needing to upload anything SHALL report a "nothing changed" outcome together with a "library changed" statement, and its detail SHALL still name how many subscriptions, folders, stars, and read marks were applied.

A status recorded by an earlier build, which carries no "library changed" statement, SHALL still be shown rather than discarded.

#### Scenario: A successful sync is reported

- **WHEN** a sync applies remote changes
- **THEN** the status line names when it ran and how many subscriptions, folders, stars, and read marks were applied

#### Scenario: A no-op sync still reports

- **WHEN** a sync finds nothing to do
- **THEN** the status line says nothing changed and when that was determined

#### Scenario: A download-only sync reports both facts

- **WHEN** a sync applies remote changes and the merged document is identical to the document it just read, so nothing is uploaded
- **THEN** the status line reports that the document on the server was unchanged and names the changes applied to the local library

#### Scenario: A failed sync keeps its reason

- **WHEN** a sync fails, and the reader opens Settings afterwards
- **THEN** the status line shows the failure reason from that sync

#### Scenario: A status from an earlier build is still shown

- **WHEN** the recorded status was written by a build that did not record whether the library changed
- **THEN** the dialog shows the recorded outcome, time, and detail as before

## ADDED Requirements

### Requirement: A sync that changed the library refreshes the view

After a sync applies changes to the local library, the application SHALL refresh the view so the reader sees them without restarting: the sidebar's subscriptions and folders, the unread counts, the entry list the reader has loaded, and the open entry. This SHALL hold whether the sync was started at startup, by the window regaining focus, or by activating "Sync now".

The decision SHALL be the recorded "library changed" statement alone, not the outcome: a sync that failed after it had already applied changes SHALL refresh, and a sync that applied nothing SHALL NOT refresh, whatever its outcome. Refreshing SHALL NOT change the reader's selection, the current filter, or how many rows were loaded, and a manual sync SHALL leave the Settings dialog open with its status line updated.

A refresh triggered this way SHALL NOT fetch the entries of newly synced subscriptions: entry bodies are re-acquired by each installation through the existing refresh actions, so a subscription imported by a sync appears in the sidebar with whatever entries this installation already holds.

#### Scenario: A download-only sync shows its changes immediately

- **WHEN** a sync imports subscriptions or folders written by another installation and nothing needs uploading
- **THEN** the sidebar and the entry counts show them as soon as the sync finishes, with no restart

#### Scenario: Imported read marks reach the list

- **WHEN** a sync imports a read mark or a star for an entry that is in the loaded list
- **THEN** that row and the unread counts reflect it as soon as the sync finishes

#### Scenario: A failure after a merge still refreshes

- **WHEN** a sync merges remote changes into the library and then fails to write the merged document back
- **THEN** the failure is reported and the view still shows the changes that were applied

#### Scenario: A manual sync refreshes without closing the dialog

- **WHEN** the reader activates "Sync now" and the sync applies changes
- **THEN** the status line shows the outcome and the view behind the dialog is refreshed, with the selection unchanged

#### Scenario: Nothing applied leaves the view alone

- **WHEN** a sync applies nothing to the local library, whether it found nothing to do or failed before merging
- **THEN** the view is not refreshed and the reader's scroll position and loaded rows are untouched

#### Scenario: A new subscription arrives without its entries

- **WHEN** a sync imports a subscription this installation has never fetched
- **THEN** the subscription is listed while its entry list stays as empty as the library is, until the reader refreshes it
