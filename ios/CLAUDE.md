# iOS App (FuzzyDrugs)

SwiftUI iPad app for veterinary drug capture and review.

## Structure

```
FuzzyDrugs/
├── FuzzyDrugsApp.swift      # App entry point
├── Audio/
│   ├── AudioRecorder.swift       # AVFoundation audio capture
│   └── TranscriptionService.swift # WhisperKit ASR integration
├── Views/
│   ├── ContentView.swift         # Main tab navigation
│   ├── RecordingView.swift       # Ambient audio capture UI
│   ├── ReviewQueueView.swift     # Pending encounters list
│   ├── EncounterDetailView.swift # Line item review/edit
│   ├── CatalogSearchView.swift   # Manual SKU lookup
│   └── SettingsView.swift        # App configuration
├── Sync/
│   └── SyncManager.swift         # PIMS sync coordination
└── Bridge/
    └── (generated UniFFI bindings)
```

## Key Components

### AudioRecorder
- Uses AVFoundation for audio capture
- Outputs 16kHz mono WAV (WhisperKit requirement)
- Provides real-time amplitude for UI feedback
- Handles microphone permissions

### TranscriptionService
- Integrates WhisperKit with `whisper-small-en` model
- CoreML/ANE acceleration for battery efficiency
- Word-level timestamps for segment alignment
- Async transcription pipeline

### ReviewQueueView
- Shows encounters sorted by lowest confidence first
- Color indicators: red (<0.5), yellow (0.5-0.7), green (>0.7)
- Tap to drill into EncounterDetailView

### EncounterDetailView
- Shows resolved line items with confidence scores
- Edit/override SKU selection
- Approve button commits to Merkle tree
- Cannot proceed without vet review

## Rust Bridge

UniFFI-generated Swift bindings in `Bridge/`:

```swift
// Factory functions
let core = try openDatabase(path: dbPath)
let core = try openDatabaseInMemory()

// Catalog operations
try core.upsertCatalogItem(item: catalogItem)
let items = try core.searchCatalog(query: "carprofen", limit: 10)

// Patient operations
let patient = try core.createPatient(name: "Max", species: "canine")

// Draft operations
let draft = try core.createDraft(patientId: patient.localId)
let pending = try core.getPendingReviewDrafts()

// Resolver
let resolved = try core.resolveMention(
    drugName: "rimadyl",
    dose: 100,
    unit: "mg",
    route: "PO",
    patientSpecies: "canine",
    patientWeightKg: 30.0
)

// Merkle commit (after vet review)
let commit = try core.commitEncounter(encounter: reviewedEncounter)

// Export
let billingJson = try core.exportBillingJson()
let complianceJson = try core.exportComplianceJson()
```

## Building

1. Generate XCFramework:
   ```bash
   ./scripts/build-xcframework.sh
   ```

2. Open in Xcode:
   ```bash
   open ios/FuzzyDrugs/FuzzyDrugs.xcodeproj
   ```

3. Add to project:
   - `Bridge/FuzzyDrugsCore.xcframework`
   - `Bridge/fuzzy_drugs_core.swift`

4. Add WhisperKit via SPM

## Dependencies

- **WhisperKit** - On-device ASR via CoreML
- **FuzzyDrugsCore.xcframework** - Rust core library

## Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     RecordingView                           │
│  [Start] → AudioRecorder → TranscriptionService → [Stop]    │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│              Rust Core (via UniFFI)                         │
│  Transcript → NER → Normalizer → Disambiguator → Draft      │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   ReviewQueueView                           │
│  Draft list sorted by confidence (lowest first)             │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                 EncounterDetailView                         │
│  Line items → Edit → [Approve] → Merkle Commit              │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    SyncManager                              │
│  Background sync to PIMS when online                        │
└─────────────────────────────────────────────────────────────┘
```

## Permissions

Required in Info.plist:
- `NSMicrophoneUsageDescription` - Audio recording
- `NSSpeechRecognitionUsageDescription` - Speech recognition (if using Apple's)
