import SwiftUI

/// View showing encounters pending vet review.
struct ReviewQueueView: View {
    @EnvironmentObject private var appState: AppState
    @State private var drafts: [DraftPreview] = []
    @State private var selectedDraft: DraftPreview?

    var body: some View {
        NavigationStack {
            Group {
                if drafts.isEmpty {
                    emptyState
                } else {
                    draftsList
                }
            }
            .navigationTitle("Review Queue")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        refreshDrafts()
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                }
            }
            .onAppear {
                refreshDrafts()
            }
            .navigationDestination(item: $selectedDraft) { draft in
                EncounterDetailView(draftId: draft.id)
            }
        }
    }

    // MARK: - Subviews

    private var emptyState: some View {
        ContentUnavailableView {
            Label("No Pending Reviews", systemImage: "checkmark.circle")
        } description: {
            Text("All encounters have been reviewed. Record a new encounter to get started.")
        }
    }

    private var draftsList: some View {
        List {
            // Low confidence section (needs attention)
            if !lowConfidenceDrafts.isEmpty {
                Section {
                    ForEach(lowConfidenceDrafts) { draft in
                        DraftRowView(draft: draft) {
                            selectedDraft = draft
                        }
                    }
                } header: {
                    Label("Needs Attention", systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)
                }
            }

            // Normal confidence section
            if !normalConfidenceDrafts.isEmpty {
                Section {
                    ForEach(normalConfidenceDrafts) { draft in
                        DraftRowView(draft: draft) {
                            selectedDraft = draft
                        }
                    }
                } header: {
                    Text("Ready for Review")
                }
            }
        }
        .listStyle(.insetGrouped)
    }

    // MARK: - Computed Properties

    private var lowConfidenceDrafts: [DraftPreview] {
        drafts.filter { ($0.lowestConfidence ?? 1.0) < 0.7 }
    }

    private var normalConfidenceDrafts: [DraftPreview] {
        drafts.filter { ($0.lowestConfidence ?? 1.0) >= 0.7 }
    }

    // MARK: - Data Loading

    private func refreshDrafts() {
        // In actual implementation:
        // drafts = (try? appState.core?.getPendingReviewDrafts().map { ... }) ?? []

        // Mock data
        drafts = [
            DraftPreview(
                id: "draft-1",
                patientName: "Max",
                patientSpecies: "Canine",
                transcript: "Give 100mg carprofen twice daily",
                pendingItems: 2,
                lowestConfidence: 0.45,
                createdAt: Date().addingTimeInterval(-3600)
            ),
            DraftPreview(
                id: "draft-2",
                patientName: "Luna",
                patientSpecies: "Feline",
                transcript: "Prescribe meloxicam for pain",
                pendingItems: 1,
                lowestConfidence: 0.92,
                createdAt: Date().addingTimeInterval(-1800)
            ),
            DraftPreview(
                id: "draft-3",
                patientName: "Buddy",
                patientSpecies: "Canine",
                transcript: "Give ace before surgery",
                pendingItems: 1,
                lowestConfidence: 0.78,
                createdAt: Date().addingTimeInterval(-600)
            ),
        ]

        appState.pendingReviewCount = drafts.count
    }
}

// MARK: - Supporting Types

struct DraftPreview: Identifiable, Hashable {
    let id: String
    let patientName: String
    let patientSpecies: String
    let transcript: String
    let pendingItems: Int
    let lowestConfidence: Double?
    let createdAt: Date
}

// MARK: - Supporting Views

struct DraftRowView: View {
    let draft: DraftPreview
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 12) {
                // Confidence indicator
                ConfidenceIndicator(confidence: draft.lowestConfidence ?? 1.0)

                // Content
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(draft.patientName)
                            .font(.headline)
                        Text("•")
                            .foregroundStyle(.secondary)
                        Text(draft.patientSpecies)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }

                    Text(draft.transcript)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)

                    HStack {
                        Label("\(draft.pendingItems) items", systemImage: "pill.fill")
                            .font(.caption2)
                            .foregroundStyle(.secondary)

                        Spacer()

                        Text(draft.createdAt, style: .relative)
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                }

                Spacer()

                Image(systemName: "chevron.right")
                    .foregroundStyle(.tertiary)
            }
            .padding(.vertical, 4)
        }
        .buttonStyle(.plain)
    }
}

struct ConfidenceIndicator: View {
    let confidence: Double

    var body: some View {
        Circle()
            .fill(confidenceColor)
            .frame(width: 12, height: 12)
    }

    private var confidenceColor: Color {
        switch confidence {
        case 0..<0.5:
            return .red
        case 0.5..<0.7:
            return .orange
        case 0.7..<0.85:
            return .yellow
        default:
            return .green
        }
    }
}

#Preview {
    ReviewQueueView()
        .environmentObject(AppState())
}
