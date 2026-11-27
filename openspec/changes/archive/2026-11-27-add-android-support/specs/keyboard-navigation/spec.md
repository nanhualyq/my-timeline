## ADDED Requirements

### Requirement: Bindings act only where a hardware keyboard is used

Keyboard bindings and the keyboard cursor SHALL act only while the application can receive hardware keyboard input: on desktop, and on a touch device with a physical keyboard attached. On a touch device without a hardware keyboard the entry list SHALL NOT depend on any binding — every action reachable by a binding SHALL also be reachable by touch — and an on-screen keyboard raised for text input SHALL NOT trigger bindings.

#### Scenario: Touch device without a keyboard

- **WHEN** the application runs on a touch device with no hardware keyboard and the reader taps and scrolls the entry list
- **THEN** no binding fires and every list action remains reachable through visible controls

#### Scenario: Hardware keyboard on a touch device

- **WHEN** a physical keyboard is attached to a device running the narrow layout and the reader presses `j`
- **THEN** the cursor moves and the next entry opens, as on desktop

#### Scenario: On-screen keyboard does not trigger bindings

- **WHEN** the reader types a feed URL in the add-feed field on a touch device
- **THEN** the typed letters change the field only and no binding fires

## MODIFIED Requirements

### Requirement: The key map is available in the application

Where the application can receive hardware keyboard input, it SHALL show a help overlay listing every binding when the reader presses `?`, and SHALL close it again on `?` or `Escape`. While the overlay is open, no other binding SHALL act on the entry list. On a touch device without a hardware keyboard, the overlay SHALL NOT be offered and no visible affordance SHALL imply that keyboard shortcuts are the way to use the application.

#### Scenario: Opening the reference

- **WHEN** the reader presses `?` with a hardware keyboard in use
- **THEN** an overlay appears listing the keyboard bindings

#### Scenario: Closing it

- **WHEN** the overlay is open and the reader presses either `?` or `Escape`
- **THEN** the overlay closes and the list is unchanged

#### Scenario: Keys do not act through the overlay

- **WHEN** the overlay is open and the reader presses `j` or `r`
- **THEN** neither the cursor nor a refresh is triggered

#### Scenario: The reference matches the bindings

- **WHEN** a binding described in the overlay is pressed
- **THEN** it performs the action the overlay describes

#### Scenario: No keyboard help on a touch-only device

- **WHEN** the application runs on a touch device with no hardware keyboard
- **THEN** no help trigger for keyboard shortcuts is shown and no user-facing instruction tells the reader to press a key
