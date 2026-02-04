import AVFoundation
import Combine

/// Audio recorder for capturing veterinary encounter audio.
/// Outputs 16kHz mono WAV format required by WhisperKit.
@MainActor
final class AudioRecorder: NSObject, ObservableObject {
    // MARK: - Published State

    @Published private(set) var isRecording = false
    @Published private(set) var currentAmplitude: Float = 0
    @Published private(set) var recordingDuration: TimeInterval = 0
    @Published private(set) var errorMessage: String?

    // MARK: - Private Properties

    private var audioEngine: AVAudioEngine?
    private var audioFile: AVAudioFile?
    private var recordingURL: URL?
    private var startTime: Date?
    private var durationTimer: Timer?

    // Audio format settings for WhisperKit
    private let sampleRate: Double = 16000
    private let channels: AVAudioChannelCount = 1

    // MARK: - Public Interface

    /// Start recording audio.
    /// - Parameter url: Optional URL to save the recording. If nil, uses a temporary file.
    /// - Returns: The URL where the recording will be saved.
    @discardableResult
    func startRecording(to url: URL? = nil) async throws -> URL {
        guard !isRecording else {
            throw AudioRecorderError.alreadyRecording
        }

        // Request microphone permission
        let permission = await requestMicrophonePermission()
        guard permission else {
            throw AudioRecorderError.permissionDenied
        }

        // Setup audio session
        try setupAudioSession()

        // Create recording URL
        let recordingURL = url ?? createTemporaryURL()
        self.recordingURL = recordingURL

        // Setup audio engine
        try setupAudioEngine(outputURL: recordingURL)

        // Start recording
        try audioEngine?.start()
        isRecording = true
        startTime = Date()
        startDurationTimer()

        return recordingURL
    }

    /// Stop recording and return the recorded file URL.
    func stopRecording() -> URL? {
        guard isRecording else { return nil }

        audioEngine?.stop()
        audioEngine?.inputNode.removeTap(onBus: 0)
        audioFile = nil

        isRecording = false
        stopDurationTimer()
        currentAmplitude = 0

        return recordingURL
    }

    /// Cancel recording and delete the file.
    func cancelRecording() {
        let url = stopRecording()
        if let url = url {
            try? FileManager.default.removeItem(at: url)
        }
    }

    // MARK: - Private Methods

    private func requestMicrophonePermission() async -> Bool {
        await withCheckedContinuation { continuation in
            AVAudioApplication.requestRecordPermission { granted in
                continuation.resume(returning: granted)
            }
        }
    }

    private func setupAudioSession() throws {
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.playAndRecord, mode: .measurement, options: [.defaultToSpeaker, .allowBluetooth])
        try session.setActive(true)
    }

    private func setupAudioEngine(outputURL: URL) throws {
        let engine = AVAudioEngine()
        let inputNode = engine.inputNode

        // Get the native format
        let inputFormat = inputNode.outputFormat(forBus: 0)

        // Create output format (16kHz mono)
        guard let outputFormat = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: sampleRate,
            channels: channels,
            interleaved: false
        ) else {
            throw AudioRecorderError.formatError
        }

        // Create converter if needed
        let converter = AVAudioConverter(from: inputFormat, to: outputFormat)

        // Create audio file
        let audioFile = try AVAudioFile(
            forWriting: outputURL,
            settings: outputFormat.settings,
            commonFormat: .pcmFormatFloat32,
            interleaved: false
        )
        self.audioFile = audioFile

        // Install tap to capture audio
        let bufferSize: AVAudioFrameCount = 4096
        inputNode.installTap(onBus: 0, bufferSize: bufferSize, format: inputFormat) { [weak self] buffer, _ in
            guard let self = self else { return }

            // Convert to output format if needed
            let outputBuffer: AVAudioPCMBuffer
            if let converter = converter {
                guard let convertedBuffer = AVAudioPCMBuffer(
                    pcmFormat: outputFormat,
                    frameCapacity: AVAudioFrameCount(Double(buffer.frameLength) * (self.sampleRate / inputFormat.sampleRate))
                ) else { return }

                var error: NSError?
                let status = converter.convert(to: convertedBuffer, error: &error) { inNumPackets, outStatus in
                    outStatus.pointee = .haveData
                    return buffer
                }

                guard status != .error else { return }
                outputBuffer = convertedBuffer
            } else {
                outputBuffer = buffer
            }

            // Calculate amplitude for visualization
            let amplitude = self.calculateAmplitude(buffer: buffer)
            Task { @MainActor in
                self.currentAmplitude = amplitude
            }

            // Write to file
            do {
                try audioFile.write(from: outputBuffer)
            } catch {
                Task { @MainActor in
                    self.errorMessage = "Failed to write audio: \(error.localizedDescription)"
                }
            }
        }

        engine.prepare()
        self.audioEngine = engine
    }

    private func calculateAmplitude(buffer: AVAudioPCMBuffer) -> Float {
        guard let channelData = buffer.floatChannelData?[0] else { return 0 }

        let frameLength = Int(buffer.frameLength)
        var sum: Float = 0

        for i in 0..<frameLength {
            sum += abs(channelData[i])
        }

        return sum / Float(frameLength)
    }

    private func createTemporaryURL() -> URL {
        let tempDir = FileManager.default.temporaryDirectory
        let filename = "recording_\(UUID().uuidString).wav"
        return tempDir.appendingPathComponent(filename)
    }

    private func startDurationTimer() {
        durationTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            guard let self = self, let startTime = self.startTime else { return }
            Task { @MainActor in
                self.recordingDuration = Date().timeIntervalSince(startTime)
            }
        }
    }

    private func stopDurationTimer() {
        durationTimer?.invalidate()
        durationTimer = nil
        recordingDuration = 0
    }
}

// MARK: - Errors

enum AudioRecorderError: LocalizedError {
    case alreadyRecording
    case permissionDenied
    case formatError
    case engineError(String)

    var errorDescription: String? {
        switch self {
        case .alreadyRecording:
            return "Recording is already in progress"
        case .permissionDenied:
            return "Microphone permission was denied"
        case .formatError:
            return "Failed to create audio format"
        case .engineError(let message):
            return "Audio engine error: \(message)"
        }
    }
}
