import SwiftUI

/// View for recording veterinary encounters.
struct RecordingView: View {
    @EnvironmentObject private var appState: AppState
    @StateObject private var recorder = AudioRecorder()
    @StateObject private var transcriptionService = TranscriptionService()

    @State private var selectedPatient: PatientSelection?
    @State private var showPatientPicker = false
    @State private var liveTranscript = ""
    @State private var showProcessingSheet = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                // Patient selection
                patientSelectionSection

                // Recording status
                recordingStatusSection

                // Live transcript preview
                transcriptPreviewSection

                Spacer()

                // Recording controls
                recordingControls
            }
            .padding()
            .navigationTitle("Record Encounter")
            .sheet(isPresented: $showPatientPicker) {
                PatientPickerSheet(selection: $selectedPatient)
            }
            .sheet(isPresented: $showProcessingSheet) {
                ProcessingSheet(transcript: liveTranscript)
            }
        }
    }

    // MARK: - Subviews

    private var patientSelectionSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Patient")
                .font(.headline)

            Button {
                showPatientPicker = true
            } label: {
                HStack {
                    if let patient = selectedPatient {
                        VStack(alignment: .leading) {
                            Text(patient.name)
                                .font(.body)
                            Text("\(patient.species) • \(patient.weight)kg")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    } else {
                        Text("Select Patient")
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .foregroundStyle(.secondary)
                }
                .padding()
                .background(Color(.secondarySystemBackground))
                .cornerRadius(10)
            }
            .buttonStyle(.plain)
        }
    }

    private var recordingStatusSection: some View {
        VStack(spacing: 16) {
            // Amplitude visualization
            AmplitudeView(amplitude: recorder.currentAmplitude, isRecording: recorder.isRecording)
                .frame(height: 100)

            // Duration
            if recorder.isRecording {
                Text(formatDuration(recorder.recordingDuration))
                    .font(.system(.title, design: .monospaced))
                    .foregroundStyle(.red)
            }
        }
    }

    private var transcriptPreviewSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Live Transcript")
                .font(.headline)

            ScrollView {
                Text(liveTranscript.isEmpty ? "Transcript will appear here..." : liveTranscript)
                    .foregroundStyle(liveTranscript.isEmpty ? .secondary : .primary)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(maxHeight: 200)
            .padding()
            .background(Color(.secondarySystemBackground))
            .cornerRadius(10)
        }
    }

    private var recordingControls: some View {
        HStack(spacing: 32) {
            // Cancel button
            Button {
                recorder.cancelRecording()
                liveTranscript = ""
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.system(size: 44))
                    .foregroundStyle(.gray)
            }
            .disabled(!recorder.isRecording)
            .opacity(recorder.isRecording ? 1 : 0.3)

            // Record/Stop button
            Button {
                Task {
                    if recorder.isRecording {
                        if let url = recorder.stopRecording() {
                            await processRecording(url: url)
                        }
                    } else {
                        try? await recorder.startRecording()
                    }
                }
            } label: {
                ZStack {
                    Circle()
                        .fill(recorder.isRecording ? .red : .blue)
                        .frame(width: 80, height: 80)

                    if recorder.isRecording {
                        RoundedRectangle(cornerRadius: 4)
                            .fill(.white)
                            .frame(width: 28, height: 28)
                    } else {
                        Circle()
                            .fill(.white)
                            .frame(width: 28, height: 28)
                    }
                }
            }
            .disabled(selectedPatient == nil)
            .opacity(selectedPatient == nil ? 0.5 : 1)

            // Done button
            Button {
                if let url = recorder.stopRecording() {
                    Task {
                        await processRecording(url: url)
                    }
                }
            } label: {
                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 44))
                    .foregroundStyle(.green)
            }
            .disabled(!recorder.isRecording)
            .opacity(recorder.isRecording ? 1 : 0.3)
        }
        .padding(.bottom, 32)
    }

    // MARK: - Helpers

    private func formatDuration(_ duration: TimeInterval) -> String {
        let minutes = Int(duration) / 60
        let seconds = Int(duration) % 60
        let tenths = Int((duration - floor(duration)) * 10)
        return String(format: "%02d:%02d.%d", minutes, seconds, tenths)
    }

    private func processRecording(url: URL) async {
        showProcessingSheet = true

        do {
            let result = try await transcriptionService.transcribe(audioURL: url)
            liveTranscript = result.text

            // Create draft and resolve mentions via Rust core
            // let draft = try appState.core?.createDraft(patientId: selectedPatient!.id)
            // ... resolve mentions and update draft

            appState.refreshPendingCount()
        } catch {
            // Handle error
            print("Transcription failed: \(error)")
        }

        showProcessingSheet = false
    }
}

// MARK: - Supporting Types

struct PatientSelection: Identifiable {
    let id: String
    let name: String
    let species: String
    let weight: Double
}

// MARK: - Supporting Views

struct AmplitudeView: View {
    let amplitude: Float
    let isRecording: Bool

    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 2) {
                ForEach(0..<50, id: \.self) { index in
                    RoundedRectangle(cornerRadius: 2)
                        .fill(barColor(for: index))
                        .frame(width: (geometry.size.width - 100) / 50)
                        .frame(height: barHeight(for: index, maxHeight: geometry.size.height))
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func barHeight(for index: Int, maxHeight: CGFloat) -> CGFloat {
        guard isRecording else { return 4 }

        let normalizedAmplitude = CGFloat(min(amplitude * 10, 1.0))
        let centerIndex = 25
        let distanceFromCenter = abs(index - centerIndex)
        let falloff = 1.0 - (CGFloat(distanceFromCenter) / 25.0)

        return max(4, normalizedAmplitude * falloff * maxHeight)
    }

    private func barColor(for index: Int) -> Color {
        guard isRecording else { return .gray.opacity(0.3) }
        return .blue
    }
}

struct PatientPickerSheet: View {
    @Binding var selection: PatientSelection?
    @Environment(\.dismiss) private var dismiss
    @State private var searchText = ""

    // Mock patient data
    let patients = [
        PatientSelection(id: "1", name: "Max", species: "Canine", weight: 30),
        PatientSelection(id: "2", name: "Luna", species: "Feline", weight: 4.5),
        PatientSelection(id: "3", name: "Buddy", species: "Canine", weight: 25),
    ]

    var body: some View {
        NavigationStack {
            List {
                ForEach(patients.filter { searchText.isEmpty || $0.name.localizedCaseInsensitiveContains(searchText) }) { patient in
                    Button {
                        selection = patient
                        dismiss()
                    } label: {
                        HStack {
                            VStack(alignment: .leading) {
                                Text(patient.name)
                                    .font(.headline)
                                Text("\(patient.species) • \(String(format: "%.1f", patient.weight))kg")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            if selection?.id == patient.id {
                                Image(systemName: "checkmark")
                                    .foregroundStyle(.blue)
                            }
                        }
                    }
                    .buttonStyle(.plain)
                }
            }
            .searchable(text: $searchText, prompt: "Search patients")
            .navigationTitle("Select Patient")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
            }
        }
    }
}

struct ProcessingSheet: View {
    let transcript: String

    var body: some View {
        VStack(spacing: 24) {
            ProgressView()
                .scaleEffect(1.5)

            Text("Processing recording...")
                .font(.headline)

            Text("Transcribing audio and resolving drug mentions")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(40)
        .interactiveDismissDisabled()
    }
}

#Preview {
    RecordingView()
        .environmentObject(AppState())
}
