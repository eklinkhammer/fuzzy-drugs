import SwiftUI

/// Settings and sync status view.
struct SettingsView: View {
    @EnvironmentObject private var appState: AppState
    @State private var isSyncing = false
    @State private var showExportSheet = false
    @State private var exportType: ExportType?

    var body: some View {
        NavigationStack {
            List {
                // Sync status section
                Section("Sync Status") {
                    HStack {
                        Image(systemName: appState.isOnline ? "wifi" : "wifi.slash")
                            .foregroundStyle(appState.isOnline ? .green : .red)
                        Text(appState.isOnline ? "Online" : "Offline")
                        Spacer()
                    }

                    if let lastSync = appState.lastSyncTime {
                        LabeledContent("Last Sync", value: lastSync, format: .relative(presentation: .named))
                    } else {
                        LabeledContent("Last Sync", value: "Never")
                    }

                    Button {
                        syncNow()
                    } label: {
                        HStack {
                            if isSyncing {
                                ProgressView()
                                    .scaleEffect(0.8)
                            }
                            Text(isSyncing ? "Syncing..." : "Sync Now")
                        }
                    }
                    .disabled(isSyncing || !appState.isOnline)
                }

                // Database info section
                Section("Database") {
                    LabeledContent("Pending Reviews", value: "\(appState.pendingReviewCount)")
                    LabeledContent("Tree Height", value: "5") // Mock
                    LabeledContent("Total Encounters", value: "127") // Mock
                }

                // Export section
                Section("Export") {
                    Button {
                        exportType = .billingJson
                        showExportSheet = true
                    } label: {
                        Label("Export Billing (JSON)", systemImage: "doc.text")
                    }

                    Button {
                        exportType = .billingCsv
                        showExportSheet = true
                    } label: {
                        Label("Export Billing (CSV)", systemImage: "tablecells")
                    }

                    Button {
                        exportType = .compliance
                        showExportSheet = true
                    } label: {
                        Label("Export Compliance Report", systemImage: "checkmark.shield")
                    }
                }

                // Model info section
                Section("Models") {
                    LabeledContent("ASR Model", value: "whisper-small-en")
                    LabeledContent("NER Model", value: "Llama-3.2-1B")
                    LabeledContent("Model Status", value: "Loaded")
                }

                // About section
                Section("About") {
                    LabeledContent("Version", value: "1.0.0")
                    LabeledContent("Build", value: "1")

                    Link(destination: URL(string: "https://example.com/privacy")!) {
                        Text("Privacy Policy")
                    }

                    Link(destination: URL(string: "https://example.com/terms")!) {
                        Text("Terms of Service")
                    }
                }
            }
            .navigationTitle("Settings")
            .sheet(isPresented: $showExportSheet) {
                if let type = exportType {
                    ExportSheet(exportType: type)
                }
            }
        }
    }

    private func syncNow() {
        isSyncing = true

        // In actual implementation:
        // Call sync manager

        Task {
            try? await Task.sleep(nanoseconds: 2_000_000_000)

            await MainActor.run {
                isSyncing = false
                appState.lastSyncTime = Date()
            }
        }
    }
}

// MARK: - Export Types

enum ExportType: String, Identifiable {
    case billingJson = "Billing (JSON)"
    case billingCsv = "Billing (CSV)"
    case compliance = "Compliance Report"

    var id: String { rawValue }
}

// MARK: - Export Sheet

struct ExportSheet: View {
    let exportType: ExportType
    @Environment(\.dismiss) private var dismiss
    @State private var isExporting = false
    @State private var exportedData: String?
    @State private var exportURL: URL?

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                if isExporting {
                    ProgressView("Generating export...")
                } else if let data = exportedData {
                    ScrollView {
                        Text(data)
                            .font(.system(.caption, design: .monospaced))
                            .padding()
                    }
                    .background(Color(.secondarySystemBackground))
                    .cornerRadius(10)
                    .padding()
                } else {
                    ContentUnavailableView {
                        Label("Export \(exportType.rawValue)", systemImage: "square.and.arrow.up")
                    } description: {
                        Text("Tap 'Generate' to create the export file.")
                    }
                }

                if let url = exportURL {
                    ShareLink(item: url) {
                        Label("Share Export", systemImage: "square.and.arrow.up")
                    }
                    .buttonStyle(.borderedProminent)
                }
            }
            .navigationTitle(exportType.rawValue)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Done") { dismiss() }
                }

                ToolbarItem(placement: .primaryAction) {
                    Button("Generate") {
                        generateExport()
                    }
                    .disabled(isExporting)
                }
            }
        }
    }

    private func generateExport() {
        isExporting = true

        // In actual implementation:
        // let data = try? appState.core?.exportBillingJson()
        // or exportBillingCsv() or exportComplianceJson()

        Task {
            try? await Task.sleep(nanoseconds: 1_000_000_000)

            await MainActor.run {
                isExporting = false

                // Mock export data
                switch exportType {
                case .billingJson:
                    exportedData = """
                    {
                      "exported_at": "2024-01-15T12:00:00Z",
                      "encounters": [
                        {
                          "draft_id": "draft-1",
                          "patient_id": "patient-1",
                          "line_items": [
                            {
                              "sku": "CARP-100",
                              "description": "Carprofen 100mg tablets",
                              "quantity": 2.0,
                              "unit": "tablets"
                            }
                          ]
                        }
                      ],
                      "total_items": 1
                    }
                    """
                case .billingCsv:
                    exportedData = """
                    draft_id,patient_id,sku,description,quantity,unit
                    draft-1,patient-1,CARP-100,Carprofen 100mg tablets,2.0,tablets
                    draft-2,patient-2,MELOX-15,Meloxicam 1.5mg/mL,0.5,mL
                    """
                case .compliance:
                    exportedData = """
                    {
                      "metadata": {
                        "format_version": "1.0",
                        "exported_at": "2024-01-15T12:00:00Z",
                        "hash_algorithm": "SHA-256",
                        "root_hash": "abc123..."
                      },
                      "encounters": [...]
                    }
                    """
                }

                // Save to temp file for sharing
                let tempDir = FileManager.default.temporaryDirectory
                let filename = "\(exportType.rawValue.replacingOccurrences(of: " ", with: "_")).txt"
                let url = tempDir.appendingPathComponent(filename)
                try? exportedData?.write(to: url, atomically: true, encoding: .utf8)
                exportURL = url
            }
        }
    }
}

#Preview {
    SettingsView()
        .environmentObject(AppState())
}
