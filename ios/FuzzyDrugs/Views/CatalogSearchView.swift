import SwiftUI

/// View for searching the drug catalog.
struct CatalogSearchView: View {
    @EnvironmentObject private var appState: AppState
    @State private var searchText = ""
    @State private var results: [CatalogDisplayItem] = []
    @State private var selectedItem: CatalogDisplayItem?

    var body: some View {
        NavigationStack {
            List {
                if results.isEmpty && !searchText.isEmpty {
                    ContentUnavailableView.search(text: searchText)
                } else if results.isEmpty {
                    Section {
                        Text("Enter a drug name, brand name, or SKU to search the catalog.")
                            .foregroundStyle(.secondary)
                    }
                } else {
                    ForEach(results) { item in
                        CatalogItemRow(item: item) {
                            selectedItem = item
                        }
                    }
                }
            }
            .searchable(text: $searchText, prompt: "Search drugs, brands, or SKUs")
            .onChange(of: searchText) { _, newValue in
                search(query: newValue)
            }
            .navigationTitle("Catalog")
            .sheet(item: $selectedItem) { item in
                CatalogItemDetailSheet(item: item)
            }
        }
    }

    private func search(query: String) {
        guard !query.isEmpty else {
            results = []
            return
        }

        // In actual implementation:
        // results = (try? appState.core?.searchCatalog(query: query, limit: 50))?.map { ... } ?? []

        // Mock data
        let allItems = [
            CatalogDisplayItem(
                sku: "CARP-100",
                name: "Carprofen 100mg tablets",
                aliases: ["Rimadyl", "Novox"],
                concentration: "100mg",
                species: ["Canine"],
                routes: ["PO"]
            ),
            CatalogDisplayItem(
                sku: "CARP-75",
                name: "Carprofen 75mg tablets",
                aliases: ["Rimadyl"],
                concentration: "75mg",
                species: ["Canine"],
                routes: ["PO"]
            ),
            CatalogDisplayItem(
                sku: "MELOX-15",
                name: "Meloxicam 1.5mg/mL oral suspension",
                aliases: ["Metacam"],
                concentration: "1.5mg/mL",
                species: ["Canine", "Feline"],
                routes: ["PO"]
            ),
            CatalogDisplayItem(
                sku: "ACE-10",
                name: "Acepromazine 10mg/mL injection",
                aliases: ["Ace", "PromAce"],
                concentration: "10mg/mL",
                species: ["Canine", "Feline", "Equine"],
                routes: ["IV", "IM", "SQ"]
            ),
            CatalogDisplayItem(
                sku: "CERENIA-24",
                name: "Cerenia 24mg tablets",
                aliases: ["Maropitant"],
                concentration: "24mg",
                species: ["Canine"],
                routes: ["PO"]
            ),
            CatalogDisplayItem(
                sku: "CONVENIA-80",
                name: "Convenia 80mg/mL injection",
                aliases: ["Cefovecin"],
                concentration: "80mg/mL",
                species: ["Canine", "Feline"],
                routes: ["SQ"]
            ),
        ]

        let queryLower = query.lowercased()
        results = allItems.filter { item in
            item.name.lowercased().contains(queryLower) ||
            item.sku.lowercased().contains(queryLower) ||
            item.aliases.contains { $0.lowercased().contains(queryLower) }
        }
    }
}

// MARK: - Supporting Types

struct CatalogDisplayItem: Identifiable {
    let id = UUID()
    let sku: String
    let name: String
    let aliases: [String]
    let concentration: String?
    let species: [String]
    let routes: [String]
}

// MARK: - Supporting Views

struct CatalogItemRow: View {
    let item: CatalogDisplayItem
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            VStack(alignment: .leading, spacing: 6) {
                Text(item.name)
                    .font(.headline)

                HStack {
                    Text(item.sku)
                        .font(.caption)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.blue.opacity(0.1))
                        .foregroundStyle(.blue)
                        .cornerRadius(4)

                    if !item.aliases.isEmpty {
                        Text("aka: \(item.aliases.joined(separator: ", "))")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }

                HStack {
                    ForEach(item.species, id: \.self) { species in
                        SpeciesTag(species: species)
                    }

                    Spacer()

                    ForEach(item.routes, id: \.self) { route in
                        Text(route)
                            .font(.caption2)
                            .padding(.horizontal, 4)
                            .padding(.vertical, 2)
                            .background(Color.gray.opacity(0.1))
                            .cornerRadius(3)
                    }
                }
            }
            .padding(.vertical, 4)
        }
        .buttonStyle(.plain)
    }
}

struct SpeciesTag: View {
    let species: String

    var body: some View {
        HStack(spacing: 2) {
            Image(systemName: iconName)
                .font(.caption2)
            Text(species)
                .font(.caption2)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2)
        .background(backgroundColor.opacity(0.1))
        .foregroundStyle(backgroundColor)
        .cornerRadius(4)
    }

    private var iconName: String {
        switch species.lowercased() {
        case "canine": return "dog.fill"
        case "feline": return "cat.fill"
        case "equine": return "figure.equestrian.sports"
        default: return "pawprint.fill"
        }
    }

    private var backgroundColor: Color {
        switch species.lowercased() {
        case "canine": return .brown
        case "feline": return .orange
        case "equine": return .purple
        default: return .gray
        }
    }
}

struct CatalogItemDetailSheet: View {
    let item: CatalogDisplayItem
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                Section("Identification") {
                    LabeledContent("SKU", value: item.sku)
                    LabeledContent("Name", value: item.name)
                    if let concentration = item.concentration {
                        LabeledContent("Concentration", value: concentration)
                    }
                }

                if !item.aliases.isEmpty {
                    Section("Also Known As") {
                        ForEach(item.aliases, id: \.self) { alias in
                            Text(alias)
                        }
                    }
                }

                Section("Compatible Species") {
                    ForEach(item.species, id: \.self) { species in
                        HStack {
                            SpeciesTag(species: species)
                            Spacer()
                        }
                    }
                }

                Section("Routes of Administration") {
                    ForEach(item.routes, id: \.self) { route in
                        HStack {
                            Text(routeFullName(route))
                            Spacer()
                            Text(route)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
            }
            .navigationTitle(item.name)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private func routeFullName(_ abbreviation: String) -> String {
        switch abbreviation {
        case "PO": return "Oral"
        case "IV": return "Intravenous"
        case "IM": return "Intramuscular"
        case "SQ": return "Subcutaneous"
        case "TOP": return "Topical"
        default: return abbreviation
        }
    }
}

#Preview {
    CatalogSearchView()
        .environmentObject(AppState())
}
