## MODIFIED Requirements

### Requirement: An entry row shows cover, title, excerpt, feed label, and time

Each entry row SHALL display, when available, a cover thumbnail, the entry title, a plain-text excerpt, the display name of the feed that provided the entry, and the entry's time. A row SHALL additionally keep its existing read indicator. A row SHALL show its starred state as a marker in the row's meta line and SHALL NOT reserve a control slot for it. Where a pointer can hover, a row SHALL additionally keep its star control; on a touch layout the row SHALL NOT render a star control, and the star SHALL be reachable by a leftward drag on the row and by the row's long-press menu. The feed display name SHALL be the same name the sidebar shows for that feed, including any custom title the reader has set.

#### Scenario: An entry with every detail available

- **WHEN** an entry with a cover image, an excerpt, and a resolvable feed is listed
- **THEN** its row shows the cover thumbnail, the title, the excerpt, the feed's display name, and the relative time, plus the star control where a pointer can hover

#### Scenario: A feed the reader has renamed

- **WHEN** an entry belongs to a feed for which the reader has set a custom title
- **THEN** the row's feed label shows that custom title, not the title supplied by the feed

#### Scenario: A starred row on a touch layout

- **WHEN** a starred entry is listed on a touch layout
- **THEN** its row shows the starred state as a marker and offers no star control

#### Scenario: The row's width goes to its content

- **WHEN** a row is listed on a touch layout
- **THEN** the title and excerpt extend to the row's trailing edge, with no space held for a control the row does not show

#### Scenario: A starred row where a pointer can hover

- **WHEN** a starred entry is listed where a pointer can hover
- **THEN** its row shows the star control in its starred appearance, and does not duplicate that state as a second marker
