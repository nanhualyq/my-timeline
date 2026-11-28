# webdav-sync Specification

## Purpose

Keeps the sync document on a WebDAV collection so a reader's shared library state moves between installations without anyone carrying a file: connection settings and where they are stored, the conditional read / merge / conditional write cycle, what counts as a change worth sending, when a sync runs, what the reader is told about it, and how the view reflects the changes a sync applied.

## Requirements

### Requirement: WebDAV connection settings are configurable and stored locally

The system SHALL let the reader configure a WebDAV server URL, a remote document path, a username, a password, and whether to accept an invalid TLS certificate. The settings, including the password, SHALL be stored in the local database; a later session SHALL use them without the reader entering them again.

The remote document path SHALL be resolved against the server URL, and a path the reader leaves empty SHALL resolve to a default document name rather than to the collection root.

#### Scenario: Settings survive a restart

- **WHEN** the reader configures a server URL, path, username, and password, and then restarts the application
- **THEN** the connection is still configured and a sync runs without re-entering anything

#### Scenario: An empty path falls back to a default

- **WHEN** the reader supplies a server URL and leaves the remote path empty
- **THEN** the system uses a default document path under that server rather than writing to the collection root

#### Scenario: Incomplete configuration disables syncing

- **WHEN** no server URL has been configured
- **THEN** no sync is attempted, automatically or manually, and the Settings dialog says the connection is not configured

### Requirement: The connection can be tested before it is relied on

The system SHALL provide a way to test the configured connection, reporting whether the server is reachable, whether the credentials are accepted, and whether the configured document exists. A failed test SHALL report the reason: unreachable host, rejected credentials, missing collection, or rejected certificate.

#### Scenario: Testing a working connection

- **WHEN** the reader tests a connection to a reachable server with valid credentials
- **THEN** the system reports that the connection succeeded and whether the document already exists there

#### Scenario: Rejected credentials are named as such

- **WHEN** the server answers the test request with an authentication failure
- **THEN** the message says the username or password was rejected, not that the host was unreachable

#### Scenario: A self-signed certificate is reported unless accepted

- **WHEN** the server presents a certificate the system does not trust, and accepting invalid certificates is switched off
- **THEN** the test fails with a certificate error, and the message names that cause

### Requirement: A sync merges before it overwrites

The system SHALL read the remote document, merge it into the local library using the sync-document rules, and upload the merged result. Uploading SHALL be conditional on the version that was read, so a document changed by another installation in the meantime is not silently overwritten: on a rejected write the system SHALL read the new version, merge again, and retry a bounded number of times before giving up and reporting the failure.

When the remote document does not exist, the system SHALL upload the current local document and create it.

#### Scenario: Local and remote changes are both kept

- **WHEN** the local library gained a star and the remote document gained a subscription since the last sync
- **THEN** after the sync the local library has both, and the remote document describes both

#### Scenario: A concurrent write is re-read rather than overwritten

- **WHEN** another installation writes the document between the read and the conditional write
- **THEN** the write is rejected, the system reads the new version, merges again, and uploads the result

#### Scenario: A missing remote document is created

- **WHEN** the remote document does not exist
- **THEN** the sync uploads the current library's document and reports that it created it

#### Scenario: Unreachable server changes nothing locally

- **WHEN** the server cannot be reached
- **THEN** the local library is unchanged and the failure is reported

### Requirement: A sync does no work when nothing changed

The system SHALL avoid downloading when the remote document is unchanged since the last sync, and SHALL avoid uploading when the merged document is identical to what it last uploaded.

For this comparison to be exact, rendering the same shared library state SHALL produce identical text: the document's records SHALL be emitted in a stable order, and re-exporting an unchanged library SHALL produce byte-identical text.

#### Scenario: Unchanged remote document is not downloaded

- **WHEN** the remote document has not changed since the last sync
- **THEN** the sync downloads nothing and reports that nothing changed

#### Scenario: Unchanged library is not uploaded

- **WHEN** neither the remote document nor the local library changed since the last sync
- **THEN** the sync writes nothing to the server

#### Scenario: Two installations with the same state render the same text

- **WHEN** two installations hold the same shared state and each exports a document
- **THEN** the two documents are byte-identical

### Requirement: A sync runs at startup, when the window regains focus, and on demand

The system SHALL run a sync once at startup, when the window regains focus (at most once per five minutes), and when the reader activates a "Sync now" action. The system SHALL NOT sync on a background timer.

A sync already in progress SHALL NOT be started a second time; a trigger that arrives during a sync SHALL be ignored rather than queued repeatedly.

#### Scenario: Startup sync runs once

- **WHEN** the application starts with a configured connection
- **THEN** one sync runs, and no further sync runs until a trigger occurs

#### Scenario: Returning to the window syncs, but not more often than the throttle

- **WHEN** the window regains focus more than once within five minutes
- **THEN** at most one sync runs in that period

#### Scenario: A running sync is not duplicated

- **WHEN** a trigger arrives while a sync is running
- **THEN** no second sync starts and the running one completes normally

#### Scenario: Manual sync always runs

- **WHEN** the reader activates "Sync now" outside the focus throttle
- **THEN** a sync runs immediately and the status line reflects its outcome

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
