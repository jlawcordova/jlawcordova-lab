import Foundation
import Testing

@testable import PixelLab

/// Categories and the Library popover's tabs, headings and keys, and the import sheets' model
/// (docs/intents/2026-10-object-libraries/spec.md, R4–R8, R12–R15, R21–R23, D5–D7).
@Suite struct ObjectLibraryTests {
    func object(_ name: String, _ category: [String]? = nil) -> ObjectSummary {
        ObjectSummary(name: name, kind: "sprite", size: [8, 8], layers: 1, frames: 1, colors: 3, description: nil, category: category)
    }

    /// pine, fir (Nature › Trees), snowpine (Nature › Trees › Pines › Snowy), rock (Nature › Stones), flowers (Nature),
    /// house (Buildings), barrel (none).
    var sample: [ObjectSummary] {
        [
            object("pine", ["Nature", "Trees"]), object("fir", ["Nature", "Trees"]),
            object("snowpine", ["Nature", "Trees", "Pines", "Snowy"]), object("rock", ["Nature", "Stones"]),
            object("flowers", ["Nature"]), object("house", ["Buildings"]), object("barrel"),
        ]
    }

    // MARK: R5 tabs

    @Test func R5_tabsAreAllThenEachOutermostCategoryThenOthers() {
        #expect(LibraryCategories.tabs(sample) == ["All", "Buildings", "Nature", "Others"])
    }

    @Test func R5_othersOnlyShowsWhenAnObjectHasNoCategory() {
        let all = sample.filter { $0.name != "barrel" }
        #expect(LibraryCategories.tabs(all) == ["All", "Buildings", "Nature"])
    }

    @Test func R5_withNoCategoriesTheRowDoesNotShowAndThePopoverIsAsItWas() {
        let plain = [object("pine"), object("rock")]
        #expect(LibraryCategories.tabs(plain).isEmpty)
        let sections = LibraryCategories.sections(plain, tab: "All")
        #expect(sections == [LibraryCategories.Section(heading: nil, objects: [object("pine"), object("rock")])])
    }

    @Test func R5_aChosenTabThatHasGoneFallsBackToAll() {
        let tabs = ["All", "Buildings", "Nature"]
        #expect(LibraryCategories.resolved("nature", in: tabs) == "Nature")
        #expect(LibraryCategories.resolved("Others", in: tabs) == "All")
        #expect(LibraryCategories.resolved(nil, in: tabs) == "All")
    }

    @Test func R2_namesComparedWithoutCaseShowWithTheFirstObjectsSpelling() {
        let objects = [object("b-pine", ["nature", "trees"]), object("a-fir", ["Nature", "Trees"])]
        #expect(LibraryCategories.tabs(objects) == ["All", "Nature"])
        let sections = LibraryCategories.sections(objects, tab: "All")
        #expect(sections.map(\.heading) == ["Nature › Trees"])
        #expect(sections[0].objects.map(\.name) == ["a-fir", "b-pine"])
    }

    // MARK: R6 headings

    @Test func R6_inACategorysTabItsOwnObjectsComeFirstThenEachSubcategoryUnderAHeading() {
        let sections = LibraryCategories.sections(sample, tab: "Nature")
        #expect(sections.map(\.heading) == [nil, "Stones", "Trees", "Trees › Pines › Snowy"])
        #expect(sections[0].objects.map(\.name) == ["flowers"])
        #expect(sections[2].objects.map(\.name) == ["fir", "pine"])
    }

    @Test func R6_inAllEveryObjectIsUnderItsFullPathAndUncategorizedComeLastUnderOthers() {
        let sections = LibraryCategories.sections(sample, tab: "All")
        #expect(sections.map(\.heading) == ["Buildings", "Nature", "Nature › Stones", "Nature › Trees", "Nature › Trees › Pines › Snowy", "Others"])
        #expect(sections.last?.objects.map(\.name) == ["barrel"])
        #expect(LibraryCategories.flattened(sections).count == sample.count)
    }

    @Test func R6_theOthersTabHoldsTheObjectsWithNoCategoryAndNoHeading() {
        let sections = LibraryCategories.sections(sample, tab: "Others")
        #expect(sections == [LibraryCategories.Section(heading: nil, objects: [object("barrel")])])
    }

    // MARK: R7 search

    @Test func R7_searchFiltersTheChosenTabAndKeepsItsHeadings() {
        let sections = LibraryCategories.sections(sample, tab: "Nature", query: "pine")
        #expect(sections.map(\.heading) == ["Trees", "Trees › Pines › Snowy"])
        #expect(LibraryCategories.flattened(sections).map(\.name) == ["pine", "snowpine"])
    }

    @Test func R7_aHeadingWithNoMatchesHides() {
        let sections = LibraryCategories.sections(sample, tab: "All", query: "rock")
        #expect(sections.map(\.heading) == ["Nature › Stones"])
    }

    @Test func R7_searchingFromAllSearchesEverything() {
        let sections = LibraryCategories.sections(sample, tab: "All", query: "r")
        #expect(LibraryCategories.flattened(sections).map(\.name).sorted() == ["barrel", "fir", "flowers", "pine", "rock", "snowpine"].filter { $0.contains("r") }.sorted())
    }

    // MARK: R8 the cursor

    @Test func R8_leftAndRightWalkReadingOrderAcrossHeadings() {
        // Sections of 3, 2 and 4 cells, 4 to a row.
        let counts = [3, 2, 4]
        #expect(LibraryCategories.move(.right, from: 2, counts: counts, columns: 4) == 3)
        #expect(LibraryCategories.move(.left, from: 3, counts: counts, columns: 4) == 2)
        #expect(LibraryCategories.move(.left, from: 0, counts: counts, columns: 4) == 0)
        #expect(LibraryCategories.move(.right, from: 8, counts: counts, columns: 4) == 8)
    }

    @Test func R8_downFromAGroupsLastRowGoesToTheNextGroupsFirstRowInTheSameColumnOrItsLastCell() {
        let counts = [3, 2, 4]
        // Column 2 of group 0 → group 1 has only columns 0 and 1: its last cell.
        #expect(LibraryCategories.move(.down, from: 2, counts: counts, columns: 4) == 4)
        // Column 1 of group 0 → column 1 of group 1.
        #expect(LibraryCategories.move(.down, from: 1, counts: counts, columns: 4) == 4)
        #expect(LibraryCategories.move(.down, from: 0, counts: counts, columns: 4) == 3)
        // The last group's last row has nothing below.
        #expect(LibraryCategories.move(.down, from: 8, counts: counts, columns: 4) == nil)
    }

    @Test func R8_downInsideAGroupWithSeveralRowsKeepsTheColumn() {
        let counts = [6, 3]
        #expect(LibraryCategories.move(.down, from: 1, counts: counts, columns: 4) == 5)
        // Column 3 of the first row: the second row has columns 0 and 1 only, so its last cell.
        #expect(LibraryCategories.move(.down, from: 3, counts: counts, columns: 4) == 5)
    }

    @Test func R8_upFromAGroupsFirstRowGoesToThePreviousGroupsLastRow() {
        let counts = [6, 3]
        #expect(LibraryCategories.move(.up, from: 6, counts: counts, columns: 4) == 4)
        #expect(LibraryCategories.move(.up, from: 8, counts: counts, columns: 4) == 5)
        #expect(LibraryCategories.move(.up, from: 5, counts: counts, columns: 4) == 1)
        // From the first row of the first group there is no cell above: the keys go to the tabs or the search.
        #expect(LibraryCategories.move(.up, from: 2, counts: counts, columns: 4) == nil)
    }

    @Test func R8_anEmptyGroupIsSkipped() {
        let counts = [2, 0, 3]
        #expect(LibraryCategories.move(.down, from: 1, counts: counts, columns: 4) == 3)
        #expect(LibraryCategories.move(.up, from: 2, counts: counts, columns: 4) == 0)
    }

    @Test func R8_control_tabStepsToTheNextAndPreviousTab() {
        let tabs = ["All", "Buildings", "Nature", "Others"]
        #expect(LibraryCategories.step("All", by: 1, in: tabs) == "Buildings")
        #expect(LibraryCategories.step("Others", by: 1, in: tabs) == "Others")
        #expect(LibraryCategories.step("Buildings", by: -1, in: tabs) == "All")
        #expect(LibraryCategories.step("All", by: -1, in: tabs) == "All")
    }

    // MARK: R4 the inspector's path

    @Test func R4_aPathIsTypedWithAnArrowOrASlash() {
        #expect(LibraryCategories.parse("Nature / Trees") == ["Nature", "Trees"])
        #expect(LibraryCategories.parse("Nature › Trees") == ["Nature", "Trees"])
        #expect(LibraryCategories.parse("  Nature/Trees/Pines ") == ["Nature", "Trees", "Pines"])
        #expect(LibraryCategories.parse("") == nil)
        #expect(LibraryCategories.parse("   ") == nil)
    }

    @Test func R4_aNameThatBreaksR2HasItsReasonAndIsntSet() {
        #expect(LibraryCategories.problem(["Nature", "Trees"]) == nil)
        #expect(LibraryCategories.problem(["Rock & Roll", "It's-fine 2"]) == nil)
        #expect(LibraryCategories.problem(["Na_ture"])?.contains("use letters, digits") == true)
        #expect(LibraryCategories.problem(["Nature", ""])?.contains("0 characters") == true)
        #expect(LibraryCategories.problem([String(repeating: "a", count: 33)])?.contains("33 characters") == true)
        #expect(LibraryCategories.problem(["Two  spaces"])?.contains("doubled space") == true)
        #expect(LibraryCategories.problem(["all"])?.contains("tab of the Library") == true)
        #expect(LibraryCategories.problem(["Others", "x"])?.contains("tab of the Library") == true)
        #expect(LibraryCategories.problem(["Nature", "All"]) == nil)
    }

    @Test func R4_theMenuListsEveryCategoryAndEveryPathAboveIt() {
        let paths = LibraryCategories.categories(sample).map { LibraryCategories.text($0) }
        #expect(paths == ["Buildings", "Nature", "Nature › Stones", "Nature › Trees", "Nature › Trees › Pines", "Nature › Trees › Pines › Snowy"])
    }

    @Test func R1_anObjectSummaryReadsItsCategoryAndNoneIsNil() throws {
        let json = #"{"objects":[{"name":"pine","kind":"sprite","category":["Nature","Trees"],"size":[8,8],"layers":1,"frames":1,"colors":3},{"name":"barrel","kind":"sprite","size":[7,9],"layers":1,"frames":1,"colors":5}]}"#
        let list = try JSONDecoder().decode(ObjectList.self, from: Data(json.utf8))
        #expect(list.objects[0].category == ["Nature", "Trees"])
        #expect(list.objects[1].category == nil)
    }

    // MARK: R12–R15 the library import sheet

    static let libraryPlanJSON = #"""
    {"library":{"path":"/x/coast.pixellib","name":"coast","exportedFrom":"harbor"},
     "objects":[{"name":"barrel","as":"barrel","fate":"new","kind":"sprite","category":null},
                {"name":"rock","as":"rock-2","fate":"renamed","kind":"sprite","category":["Nature","Stones"]},
                {"name":"pine","as":"pine","fate":"same","kind":"sprite","category":["Nature","Trees"]}],
     "count":2,"renames":[{"from":"rock","to":"rock-2"}],"same":["pine"],
     "differing":[{"name":"water-3","tier":"world","library":"#5F8C7E","document":"#4F7F9A"}],
     "character":{"swapped":false,"figures":[]},
     "missing":[{"name":"gold-4","tier":"world","hex":"#7A5E2A","nearest":{"name":"wood-3","hex":"#7A4E2D"},"usedBy":["hall-roof"]}],
     "needsAcceptDifferences":true,"ready":false,"dryRun":true,"summary":"Would import 2 objects from coast; nothing changed"}
    """#

    @Test func R12_theDryRunPlanDecodes() throws {
        let plan = try JSONDecoder().decode(LibraryPlan.self, from: Data(Self.libraryPlanJSON.utf8))
        #expect(plan.count == 2)
        #expect(plan.renames == [LibraryPlan.Rename(from: "rock", to: "rock-2")])
        #expect(plan.objects[1].fate == "renamed")
        #expect(plan.missing[0].nearest?.name == "wood-3")
        #expect(plan.changed == nil)
    }

    @Test func R13_aRenameDifferenceOrMissingColorNeedsTheSheetAndAnythingElseImportsAtOnce() throws {
        var plan = try JSONDecoder().decode(LibraryPlan.self, from: Data(Self.libraryPlanJSON.utf8))
        var draft = LibraryDraft(path: "/x/coast.pixellib", plan: plan, conversions: [:])
        #expect(draft.needsSheet)
        plan.renames = []
        plan.differing = []
        plan.missing = []
        draft.plan = plan
        #expect(!draft.needsSheet)
        plan.renames = [.init(from: "a", to: "a-2")]
        draft.plan = plan
        #expect(draft.needsSheet)
        #expect(draft.buttonTitle == "Import")
    }

    @Test func R14_differencesAndConversionsMakeTheButtonImportAnyway() throws {
        let plan = try JSONDecoder().decode(LibraryPlan.self, from: Data(Self.libraryPlanJSON.utf8))
        let draft = LibraryDraft(path: "/x/coast.pixellib", plan: plan, conversions: [:])
        #expect(draft.hasDifferences)
        #expect(draft.buttonTitle == "Import Anyway")
        var swapped = plan
        swapped.differing = []
        swapped.missing = []
        swapped.character = .init(swapped: true, figures: ["chef"])
        #expect(LibraryDraft(path: "/x", plan: swapped, conversions: [:]).buttonTitle == "Import Anyway")
    }

    @Test func R14_theSheetSaysWhetherTheDocumentsColorIsDarkerOrLighter() {
        #expect(LibraryDraft.lookWord(document: "#4F7F9A", library: "#5F8C7E") == "darker")
        #expect(LibraryDraft.lookWord(document: "#FFFFFF", library: "#000000") == "lighter")
        #expect(LibraryDraft.lookWord(document: "#808080", library: "#818181") == "a different shade")
    }

    @Test func R15_aMissingColorOffersTheSameTierWithTheNearestFirst() throws {
        let plan = try JSONDecoder().decode(LibraryPlan.self, from: Data(Self.libraryPlanJSON.utf8))
        let palette = [
            PaletteColor(name: "grass-1", hex: "#00AA00", tier: "world"), PaletteColor(name: "wood-3", hex: "#7A4E2D", tier: "world"),
            PaletteColor(name: "gold-3", hex: "#B09040", tier: "world"), PaletteColor(name: "shirt-1", hex: "#112233", tier: "outfit"),
        ]
        let options = LibraryDraft.options(for: plan.missing[0], palette: palette)
        #expect(options.map(\.name) == ["wood-3", "grass-1", "gold-3"])
    }

    // MARK: R21–R23 the palette import sheet

    static let palettePlanJSON = #"""
    {"file":{"path":"/x/coast.pixelpal","name":"coast"},
     "new":[{"name":"gold-4","tier":"world","hex":"#7A5E2A","selected":true},{"name":"sky-5","tier":"world","hex":"#99BBEE","selected":true}],
     "differing":[{"name":"water-3","tier":"world","theirs":"#5F8C7E","mine":"#4F7F9A"}],
     "otherTier":[],
     "tiers":{"world":{"have":31,"adding":2,"cap":32,"over":1},"outfit":{"have":16,"adding":0,"cap":16,"over":0}},
     "over":1,"ready":false}
    """#

    @Test func R23_whenTheNewColorsPassACapImportStaysOffUntilTheyFit() throws {
        let plan = try JSONDecoder().decode(PalettePlan.self, from: Data(Self.palettePlanJSON.utf8))
        var draft = PaletteDraft(path: "/x/coast.pixelpal", plan: plan)
        #expect(draft.overTotal == 1)
        #expect(!draft.canImport)
        draft.selected.remove("sky-5")
        #expect(draft.overTotal == 0)
        #expect(draft.narrowed)
        #expect(draft.canImport)
    }

    @Test func R22_aDifferingColorKeepsMineUnlessTheOwnerChoosesTheirs() throws {
        let plan = try JSONDecoder().decode(PalettePlan.self, from: Data(Self.palettePlanJSON.utf8))
        var draft = PaletteDraft(path: "/x/coast.pixelpal", plan: plan)
        #expect(draft.useTheirs.isEmpty)
        draft.useTheirs.insert("water-3")
        #expect(draft.useTheirs == ["water-3"])
    }

    @Test func R21_withRoomEveryNewColorIsSelectedAndNothingIsNarrowed() throws {
        var plan = try JSONDecoder().decode(PalettePlan.self, from: Data(Self.palettePlanJSON.utf8))
        plan.tiers.world = .init(have: 20, adding: 2, cap: 32, over: 0)
        let draft = PaletteDraft(path: "/x", plan: plan)
        #expect(draft.overTotal == 0)
        #expect(!draft.narrowed)
        #expect(draft.canImport)
    }

    @Test func R18_aLibraryOrPaletteFileIsRecognizedByItsExtension() {
        #expect(ImportModel.isLibrary("/a/coast.pixellib"))
        #expect(ImportModel.isPalette("/a/Coast.PIXELPAL"))
        #expect(!ImportModel.isLibrary("/a/harbor.pixellab"))
    }
}
