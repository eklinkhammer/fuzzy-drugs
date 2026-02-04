import SwiftUI

/// Main content view with tab navigation.
struct ContentView: View {
    @StateObject private var appState = AppState()

    var body: some View {
        TabView {
            RecordingView()
                .tabItem {
                    Label("Record", systemImage: "mic.fill")
                }
                .environmentObject(appState)

            ReviewQueueView()
                .tabItem {
                    Label("Review", systemImage: "checklist")
                }
                .badge(appState.pendingReviewCount)
                .environmentObject(appState)

            CatalogSearchView()
                .tabItem {
                    Label("Catalog", systemImage: "magnifyingglass")
                }
                .environmentObject(appState)

            SettingsView()
                .tabItem {
                    Label("Settings", systemImage: "gear")
                }
                .environmentObject(appState)
        }
    }
}

/// Global app state.
@MainActor
final class AppState: ObservableObject {
    @Published var pendingReviewCount: Int = 0
    @Published var isOnline: Bool = true
    @Published var lastSyncTime: Date?

    // Reference to Rust core (would be initialized with actual path)
    // var core: FuzzyDrugsCore?

    init() {
        // Initialize core
        // core = try? FuzzyDrugsCore.open(path: getDatabasePath())
        refreshPendingCount()
    }

    func refreshPendingCount() {
        // pendingReviewCount = (try? core?.getPendingReviewDrafts().count) ?? 0
        pendingReviewCount = 3 // Placeholder
    }

    private func getDatabasePath() -> String {
        let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        return documentsPath.appendingPathComponent("fuzzy_drugs.db").path
    }
}

#Preview {
    ContentView()
}
