import Foundation

enum DocumentFormat: String, CaseIterable, Identifiable, Codable, Sendable {
    case screenplay, comic, novel, academic, stageplay, audio, general, poetry
    var id: String { rawValue }
    var name: String {
        switch self { case .screenplay: "Screenplay"; case .comic: "Comic Script"; case .novel: "Book"; case .academic: "Academic Paper"; case .stageplay: "Stage Play"; case .audio: "Audio Script"; case .general: "General Writing"; case .poetry: "Poetry & Lyrics" }
    }
    var code: UInt32 { switch self { case .screenplay: 0; case .comic: 1; case .novel: 2; case .academic: 3; case .stageplay: 4; case .audio: 5; case .general: 6; case .poetry: 7 } }
    var unit: String { switch self { case .screenplay: "scene"; case .comic: "page"; case .novel: "chapter"; case .academic, .audio, .general: "section"; case .stageplay: "scene"; case .poetry: "stanza" } }
    var hasCast: Bool { [.screenplay, .comic, .stageplay, .audio].contains(self) }
    var structureHint: String {
        switch self {
        case .screenplay: "Scene headings will appear as you write."
        case .comic: "Page and panel headings will appear as you write."
        case .novel: "Chapter headings will appear as you write."
        case .academic, .general: "Use # headings to organize your writing."
        case .stageplay: "Act and scene headings will appear as you write."
        case .audio: "Scene and segment headings will appear as you write."
        case .poetry: "Use # titles or verse and chorus labels to navigate your work."
        }
    }
    var sourceFilename: String { self == .screenplay ? "text.fountain" : "text.txt" }
    var sourceType: String { self == .screenplay ? "com.fountain.screenplay" : "public.plain-text" }
    var presets: [LayoutPreset] {
        switch self {
        case .screenplay: [.screenplay]
        case .comic: [.comicFull, .comicLettering]
        case .novel: [.novelManuscript, .novelReading]
        case .academic: [.academicDraft]
        case .stageplay: [.stageDraft]
        case .audio: [.audioDrama, .podcast]
        case .general: [.generalDraft, .generalOutline]
        case .poetry: [.poetry, .lyrics]
        }
    }
    var prompt: String {
        switch self {
        case .screenplay: "INT. SOMEWHERE WONDERFUL - DAY"
        case .comic: "PAGE 1 · PANEL 1 · A world waiting to be drawn."
        case .novel: "Chapter One"
        case .academic: "# Introduction"
        case .stageplay: "ACT I · SCENE 1"
        case .audio: "SCENE 1 · A voice in the silence."
        case .general: "An idea worth putting into words."
        case .poetry: "Give each line room to breathe."
        }
    }
    var starter: String {
        switch self {
        case .screenplay: "Title: Untitled Screenplay\nAuthor: \n\nINT. ROOM - DAY\n\nDescribe the action.\n\nCHARACTER\nThe first line of dialogue.\n"
        case .comic: "Title: Untitled Comic\nIssue: 1\nAuthor: \n\nPAGE 1 (2 panels)\n\nPanel 1\nDescribe one drawable moment: the setting, characters, and action.\n\nCHARACTER:\nThe first line of dialogue.\n\nCAPTION:\nA moment earlier.\n\nPanel 2\nDescribe the next moment.\n\nSFX:\nCLICK\n"
        case .novel: "Title: Untitled Book\nAuthor: \n\nChapter One\n\nBegin your story here.\n\nThe next paragraph.\n\n***\n\nA new scene begins.\n"
        case .stageplay: "Title: Untitled Play\nAuthor: \n\nACT I\n\nSCENE 1\n\n(A room at dawn. Describe the setting and opening action.)\n\nCHARACTER\nThe first line of dialogue.\n\n(An action between lines.)\n\nCHARACTER\nThe next line of dialogue.\n"
        case .audio: "Title: Untitled Audio Drama\nAuthor: \n\nSCENE 1 - THE ARRIVAL\n\nSFX:\nFootsteps approach, then stop.\n\nCHARACTER:\nThe first line of dialogue.\n\nMUSIC:\nA brief transition.\n\nSCENE 2 - A NEW VOICE\n\nCHARACTER:\nThe next line of dialogue.\n"
        case .general: "Title: Untitled Draft\nAuthor: \n\n# An idea\n\nBegin writing here.\n\n# A new direction\n\nContinue your thought.\n"
        case .poetry: "Title: Untitled Collection\nAuthor: \n\n# First Poem\n\nBegin a line here.\nLet the next line find its rhythm.\n\nA new stanza begins.\n"
        case .academic: "Title: Untitled Paper\nAuthor: \n\n# Introduction\n\nIntroduce your question and central argument.\n\n# Discussion\n\nDevelop your argument and record your sources.\n\n# Conclusion\n\nBring your findings together.\n"
        }
    }
    func starter(for preset: LayoutPreset) -> String {
        switch preset {
        case .podcast:
            "Title: Untitled Podcast\nAuthor: \n\nSEGMENT 1 - INTRODUCTION\n\nMUSIC:\nOpening theme, then fade under.\n\nHOST:\nWelcome to the show.\n\nSEGMENT 2 - CONVERSATION\n\nHOST:\nThe first question.\n\nGUEST:\nThe first answer.\n"
        case .lyrics:
            "Title: Untitled Song\nAuthor: \n\nVERSE 1\nThe first line of your verse.\nThe next line.\n\nCHORUS\nThe words we come back to.\n\nVERSE 2\nA new thought begins.\n\nBRIDGE\nA change in direction.\n"
        case .generalOutline:
            "Title: Untitled Outline\nAuthor: \n\n# The idea\n\nDescribe the central idea.\n\n## Key points\n\n- First point\n- Second point\n\n# Next steps\n\nWhat happens next?\n"
        default: starter
        }
    }
}

enum LayoutPreset: String, CaseIterable, Identifiable, Sendable {
    case screenplay = "screenplay-standard"
    case comicFull = "comic-full-script"
    case comicLettering = "comic-lettering-friendly"
    case novelManuscript = "novel-manuscript"
    case novelReading = "novel-reading"
    case academicDraft = "academic-draft"
    case stageDraft = "stage-draft"
    case audioDrama = "audio-drama"
    case podcast = "audio-podcast"
    case generalDraft = "general-draft"
    case generalOutline = "general-outline"
    case poetry = "poetry-standard"
    case lyrics = "poetry-lyrics"
    var id: String { rawValue }
    var name: String {
        switch self {
        case .screenplay: "Screenplay"
        case .comicFull: "Fountage Standard"
        case .comicLettering: "Lettering Friendly"
        case .novelManuscript: "Manuscript"
        case .novelReading: "Reading Draft"
        case .academicDraft: "Academic Draft"
        case .stageDraft: "Stage Draft"
        case .audioDrama: "Audio Drama"
        case .podcast: "Podcast"
        case .generalDraft: "Free Writing"
        case .generalOutline: "Outline"
        case .poetry: "Poetry"
        case .lyrics: "Song Lyrics"
        }
    }
}

// A requested standard, not a claim that the draft renderer implements its rules.
// Keep edition explicit so an application update never silently migrates a paper.
struct AcademicStyle: Equatable, Sendable {
    var family: String
    var edition: Int
    var variant: String
    var name: String {
        let variantName = variant == "student" ? "Student" : variant == "professional" ? "Professional" : variant
        return "\(family.uppercased()) \(edition)" + (variantName.isEmpty ? "" : " · \(variantName)")
    }
    static let apa7Student = Self(family: "apa", edition: 7, variant: "student")
    static let apa7Professional = Self(family: "apa", edition: 7, variant: "professional")
    static let mla9 = Self(family: "mla", edition: 9, variant: "")
    static let choices: [Self] = [.apa7Student, .apa7Professional, .mla9]
}
