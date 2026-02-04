import SwiftUI

/// Detailed view for reviewing and approving an encounter.
struct EncounterDetailView: View {
    let draftId: String
    @EnvironmentObject private var appState: AppState
    @Environment(\.dismiss) private var dismiss

    @State private var resolvedItems: [ResolvedItemView] = []
    @State private var transcript = ""
    @State private var showCatalogSearch = false
    @State private var editingItemIndex: Int?
    @State private var isSubmitting = false

    var body: some View {
        List {
            // Transcript section
            Section("Transcript") {
                Text(transcript)
                    .font(.body)
            }

            // Resolved items section
            Section("Drug Items") {
                ForEach(resolvedItems.indices, id: \.self) { index in
                    ResolvedItemRow(
                        item: $resolvedItems[index],
                        onSelectAlternative: {
                            editingItemIndex = index
                            showCatalogSearch = true
                        }
                    )
                }
            }

            // Add item button
            Section {
                Button {
                    showCatalogSearch = true
                    editingItemIndex = nil
                } label: {
                    Label("Add Item Manually", systemImage: "plus.circle")
                }
            }
        }
        .navigationTitle("Review Encounter")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("Approve") {
                    approveEncounter()
                }
                .disabled(!allItemsReviewed || isSubmitting)
            }
        }
        .sheet(isPresented: $showCatalogSearch) {
            CatalogSearchSheet { selectedItem in
                if let index = editingItemIndex {
                    resolvedItems[index].selectedSku = selectedItem.sku
                    resolvedItems[index].selectedName = selectedItem.name
                    resolvedItems[index].status = .alternativeSelected
                } else {
                    // Adding new item
                    resolvedItems.append(ResolvedItemView(
                        originalMention: "Manual entry",
                        normalizedName: selectedItem.name,
                        selectedSku: selectedItem.sku,
                        selectedName: selectedItem.name,
                        confidence: 1.0,
                        alternatives: [],
                        status: .approved
                    ))
                }
            }
        }
        .onAppear {
            loadDraft()
        }
    }

    // MARK: - Computed Properties

    private var allItemsReviewed: Bool {
        resolvedItems.allSatisfy { $0.status != .pending }
    }

    // MARK: - Data Loading

    private func loadDraft() {
        // In actual implementation:
        // let draft = try? appState.core?.getDraft(draftId: draftId)
        // transcript = draft?.transcript ?? ""
        // resolvedItems = draft?.resolvedItems.map { ... } ?? []

        // Mock data
        transcript = "Give the dog 100mg of carprofen twice daily by mouth. Also prescribe metacam 0.5mL for pain management."

        resolvedItems = [
            ResolvedItemView(
                originalMention: "100mg of carprofen twice daily by mouth",
                normalizedName: "carprofen",
                selectedSku: "CARP-100",
                selectedName: "Carprofen 100mg tablets",
                confidence: 0.95,
                alternatives: [
                    AlternativeItem(sku: "CARP-75", name: "Carprofen 75mg tablets", confidence: 0.82),
                    AlternativeItem(sku: "CARP-25", name: "Carprofen 25mg tablets", confidence: 0.65),
                ],
                status: .pending
            ),
            ResolvedItemView(
                originalMention: "metacam 0.5mL",
                normalizedName: "meloxicam",
                selectedSku: "MELOX-15",
                selectedName: "Meloxicam 1.5mg/mL oral suspension",
                confidence: 0.45,
                alternatives: [
                    AlternativeItem(sku: "MELOX-05", name: "Meloxicam 0.5mg/mL injectable", confidence: 0.38),
                ],
                status: .pending
            ),
        ]
    }

    private func approveEncounter() {
        isSubmitting = true

        // In actual implementation:
        // Create ReviewedEncounter and commit to Merkle tree
        // try? appState.core?.commitEncounter(...)

        Task {
            try? await Task.sleep(nanoseconds: 500_000_000)

            await MainActor.run {
                appState.refreshPendingCount()
                dismiss()
            }
        }
    }
}

// MARK: - Supporting Types

struct ResolvedItemView: Identifiable {
    let id = UUID()
    let originalMention: String
    let normalizedName: String
    var selectedSku: String
    var selectedName: String
    let confidence: Double
    let alternatives: [AlternativeItem]
    var status: ItemReviewStatus
}

struct AlternativeItem: Identifiable {
    let id = UUID()
    let sku: String
    let name: String
    let confidence: Double
}

enum ItemReviewStatus {
    case pending
    case approved
    case alternativeSelected
    case rejected
}

// MARK: - Supporting Views

struct ResolvedItemRow: View {
    @Binding var item: ResolvedItemView
    let onSelectAlternative: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Original mention
            HStack {
                Text("\"" + item.originalMention + "\"")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .italic()
                Spacer()
                ConfidenceIndicator(confidence: item.confidence)
            }

            // Selected item
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(item.selectedName)
                        .font(.headline)
                    Text(item.selectedSku)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                statusIcon
            }

            // Action buttons
            HStack(spacing: 16) {
                // Approve button
                Button {
                    item.status = .approved
                } label: {
                    Label("Approve", systemImage: "checkmark")
                        .font(.caption)
                }
                .buttonStyle(.bordered)
                .tint(.green)
                .disabled(item.status == .approved)

                // Select alternative
                if !item.alternatives.isEmpty {
                    Menu {
                        ForEach(item.alternatives) { alt in
                            Button {
                                item.selectedSku = alt.sku
                                item.selectedName = alt.name
                                item.status = .alternativeSelected
                            } label: {
                                VStack(alignment: .leading) {
                                    Text(alt.name)
                                    Text("\(Int(alt.confidence * 100))% match")
                                }
                            }
                        }

                        Divider()

                        Button {
                            onSelectAlternative()
                        } label: {
                            Label("Search Catalog...", systemImage: "magnifyingglass")
                        }
                    } label: {
                        Label("Alternatives", systemImage: "arrow.triangle.swap")
                            .font(.caption)
                    }
                    .buttonStyle(.bordered)
                }

                // Reject button
                Button {
                    item.status = .rejected
                } label: {
                    Label("Reject", systemImage: "xmark")
                        .font(.caption)
                }
                .buttonStyle(.bordered)
                .tint(.red)
                .disabled(item.status == .rejected)
            }
        }
        .padding(.vertical, 8)
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch item.status {
        case .pending:
            EmptyView()
        case .approved:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
        case .alternativeSelected:
            Image(systemName: "arrow.triangle.swap")
                .foregroundStyle(.blue)
        case .rejected:
            Image(systemName: "xmark.circle.fill")
                .foregroundStyle(.red)
        }
    }
}

struct CatalogSearchSheet: View {
    @Environment(\.dismiss) private var dismiss
    let onSelect: (CatalogSearchResult) -> Void

    @State private var searchText = ""
    @State private var results: [CatalogSearchResult] = []

    var body: some View {
        NavigationStack {
            List {
                ForEach(results) { result in
                    Button {
                        onSelect(result)
                        dismiss()
                    } label: {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(result.name)
                                .font(.headline)
                            HStack {
                                Text(result.sku)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                if !result.species.isEmpty {
                                    Text("•")
                                        .foregroundStyle(.secondary)
                                    Text(result.species.joined(separator: ", "))
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
            .searchable(text: $searchText, prompt: "Search catalog")
            .onChange(of: searchText) { _, newValue in
                search(query: newValue)
            }
            .navigationTitle("Search Catalog")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }

    private func search(query: String) {
        guard !query.isEmpty else {
            results = []
            return
        }

        // In actual implementation:
        // results = (try? appState.core?.searchCatalog(query: query, limit: 20)) ?? []

        // Mock data
        results = [
            CatalogSearchResult(sku: "CARP-100", name: "Carprofen 100mg tablets", species: ["Canine"]),
            CatalogSearchResult(sku: "CARP-75", name: "Carprofen 75mg tablets", species: ["Canine"]),
            CatalogSearchResult(sku: "MELOX-15", name: "Meloxicam 1.5mg/mL oral suspension", species: ["Canine", "Feline"]),
        ].filter { $0.name.localizedCaseInsensitiveContains(query) || $0.sku.localizedCaseInsensitiveContains(query) }
    }
}

struct CatalogSearchResult: Identifiable {
    let id = UUID()
    let sku: String
    let name: String
    let species: [String]
}

#Preview {
    NavigationStack {
        EncounterDetailView(draftId: "draft-1")
            .environmentObject(AppState())
    }
}
