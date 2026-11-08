#import <Cocoa/Cocoa.h>

typedef void (*SC2KMenuCallback)(int);
typedef void (*SC2KDocumentOpenCallback)(const char *);

static SC2KMenuCallback gCallback = NULL;
static SC2KDocumentOpenCallback gDocumentOpenCallback = NULL;
static NSMenuItem *gAppearanceSystemItem = nil;
static NSMenuItem *gAppearanceLightItem = nil;
static NSMenuItem *gAppearanceDarkItem = nil;

enum {
    SC2KCommandOpenCity = 1,
    SC2KCommandReloadAssets = 2,
    SC2KCommandZoomIn = 3,
    SC2KCommandZoomOut = 4,
    SC2KCommandActualSize = 5,
    SC2KCommandCenterMap = 6,
    SC2KCommandToggleGrid = 7,
    SC2KCommandInspectTile = 8,
    SC2KCommandToggleZoneOverlay = 9,
    SC2KCommandFitMap = 10,
    SC2KCommandToggleTraffic = 11,
    SC2KCommandInspectHoverTile = 12,
    SC2KCommandSaveCity = 13,
    SC2KCommandSaveCityAs = 14,
    SC2KCommandNewCity = 15,
    SC2KCommandToggleEditMap = 16,
    SC2KCommandLoadTileSet = 17,
    SC2KCommandLoadScenario = 18,
    SC2KCommandReturnToTitle = 19,
    SC2KCommandSpeedPause = 20,
    SC2KCommandSpeedTurtle = 21,
    SC2KCommandSpeedLlama = 22,
    SC2KCommandSpeedCheetah = 23,
    SC2KCommandSpeedSwallow = 24,
    SC2KCommandRotateCcw = 30,
    SC2KCommandRotateCw = 31,
    SC2KCommandOpenMapWindow = 40,
    SC2KCommandOpenBudgetWindow = 41,
    SC2KCommandOpenPopulationWindow = 42,
    SC2KCommandOpenIndustryWindow = 43,
    SC2KCommandOpenGraphsWindow = 44,
    SC2KCommandOpenNeighborsWindow = 45,
    SC2KCommandOpenOrdinancesWindow = 46,
    SC2KCommandOpenScenarioProgressWindow = 47,
    SC2KCommandDataMapCity = 110,
    SC2KCommandDataMapPower = 111,
    SC2KCommandDataMapWater = 112,
    SC2KCommandDataMapTraffic = 113,
    SC2KCommandDataMapLandValue = 114,
    SC2KCommandToggleUnderground = 115,
    SC2KCommandDataMapPollution = 116,
    SC2KCommandDataMapCrime = 117,
    SC2KCommandDataMapPolice = 118,
    SC2KCommandDataMapFire = 119,
    SC2KCommandDataMapDensity = 120,
    SC2KCommandDataMapGrowth = 121,
    SC2KCommandDataMapHeight = 122,
    SC2KCommandToggleAutoBudget = 50,
    SC2KCommandToggleAutoGoto = 51,
    SC2KCommandToggleSoundEffects = 52,
    SC2KCommandToggleMusic = 53,
    SC2KCommandImportOriginalAssets = 90,
    SC2KCommandChooseOriginalAssets = 91,
    SC2KCommandChooseReplacementAssets = 92,
    SC2KCommandPreferOriginalGraphics = 93,
    SC2KCommandPreferReplacementGraphics = 94,
    SC2KCommandNextGraphicsSource = 95,
    SC2KCommandNextEffectsSource = 96,
    SC2KCommandNextMusicSource = 97,
    SC2KCommandToggleCityGameEffects = 98,
    SC2KCommandToggleCityGameMusic = 99,
    SC2KCommandToggleCityGameCityMusic = 103,
    SC2KCommandToggleCityGameToolbar = 101,
    SC2KCommandToggleCityGameNewspaper = 102,
    SC2KCommandAppearanceSystem = 130,
    SC2KCommandAppearanceLight = 131,
    SC2KCommandAppearanceDark = 132,
    SC2KCommandToggleNoDisasters = 60,
    SC2KCommandStartFire = 61,
    SC2KCommandStartFlood = 62,
    SC2KCommandStartAirCrash = 63,
    SC2KCommandStartTornado = 64,
    SC2KCommandStartEarthquake = 65,
    SC2KCommandStartMonster = 66,
    SC2KCommandStartHurricane = 67,
    SC2KCommandStartRioters = 68,
    SC2KCommandOpenNewspaper = 70,
    SC2KCommandOpenCourier = 71,
    SC2KCommandOpenExtra = 72,
    SC2KCommandOpenHelp = 80,
    SC2KCommandToggleBalloonHelp = 81
};

@interface SC2KMenuTarget : NSObject <NSApplicationDelegate>
@end

@implementation SC2KMenuTarget

- (void)sendCommand:(NSInteger)command {
    if (gCallback != NULL) {
        gCallback((int)command);
    }
}

- (void)about:(id)sender {
    NSAlert *alert = [[NSAlert alloc] init];
    alert.messageText = @"CityGame";
    alert.informativeText = @"A city-building game compatible with SimCity 2000 cities and assets.";
    [alert addButtonWithTitle:@"OK"];
    [alert runModal];
}

- (void)application:(NSApplication *)application openFiles:(NSArray<NSString *> *)filenames {
    NSString *path = filenames.firstObject;
    if (path != nil && gDocumentOpenCallback != NULL) {
        gDocumentOpenCallback([path fileSystemRepresentation]);
        [application replyToOpenOrPrint:NSApplicationDelegateReplySuccess];
    } else {
        [application replyToOpenOrPrint:NSApplicationDelegateReplyFailure];
    }
}

- (void)openCity:(id)sender { [self sendCommand:SC2KCommandOpenCity]; }
- (void)reloadAssets:(id)sender { [self sendCommand:SC2KCommandReloadAssets]; }
- (void)zoomIn:(id)sender { [self sendCommand:SC2KCommandZoomIn]; }
- (void)zoomOut:(id)sender { [self sendCommand:SC2KCommandZoomOut]; }
- (void)actualSize:(id)sender { [self sendCommand:SC2KCommandActualSize]; }
- (void)centerMap:(id)sender { [self sendCommand:SC2KCommandCenterMap]; }
- (void)toggleGrid:(id)sender { [self sendCommand:SC2KCommandToggleGrid]; }
- (void)toggleZoneOverlay:(id)sender { [self sendCommand:SC2KCommandToggleZoneOverlay]; }
- (void)fitMap:(id)sender { [self sendCommand:SC2KCommandFitMap]; }
- (void)toggleTraffic:(id)sender { [self sendCommand:SC2KCommandToggleTraffic]; }
- (void)inspectTile:(id)sender { [self sendCommand:SC2KCommandInspectTile]; }
- (void)inspectHoverTile:(id)sender { [self sendCommand:SC2KCommandInspectHoverTile]; }
- (void)saveCity:(id)sender { [self sendCommand:SC2KCommandSaveCity]; }
- (void)saveCityAs:(id)sender { [self sendCommand:SC2KCommandSaveCityAs]; }
- (void)newCity:(id)sender { [self sendCommand:SC2KCommandNewCity]; }
- (void)returnToTitle:(id)sender { [self sendCommand:SC2KCommandReturnToTitle]; }
- (void)toggleEditMap:(id)sender { [self sendCommand:SC2KCommandToggleEditMap]; }
- (void)loadTileSet:(id)sender { [self sendCommand:SC2KCommandLoadTileSet]; }
- (void)loadScenario:(id)sender { [self sendCommand:SC2KCommandLoadScenario]; }
- (void)speedPause:(id)sender { [self sendCommand:SC2KCommandSpeedPause]; }
- (void)speedTurtle:(id)sender { [self sendCommand:SC2KCommandSpeedTurtle]; }
- (void)speedLlama:(id)sender { [self sendCommand:SC2KCommandSpeedLlama]; }
- (void)speedCheetah:(id)sender { [self sendCommand:SC2KCommandSpeedCheetah]; }
- (void)speedSwallow:(id)sender { [self sendCommand:SC2KCommandSpeedSwallow]; }
- (void)rotateCcw:(id)sender { [self sendCommand:SC2KCommandRotateCcw]; }
- (void)rotateCw:(id)sender { [self sendCommand:SC2KCommandRotateCw]; }
- (void)openMapWindow:(id)sender { [self sendCommand:SC2KCommandOpenMapWindow]; }
- (void)openBudgetWindow:(id)sender { [self sendCommand:SC2KCommandOpenBudgetWindow]; }
- (void)openPopulationWindow:(id)sender { [self sendCommand:SC2KCommandOpenPopulationWindow]; }
- (void)openIndustryWindow:(id)sender { [self sendCommand:SC2KCommandOpenIndustryWindow]; }
- (void)openGraphsWindow:(id)sender { [self sendCommand:SC2KCommandOpenGraphsWindow]; }
- (void)openNeighborsWindow:(id)sender { [self sendCommand:SC2KCommandOpenNeighborsWindow]; }
- (void)openOrdinancesWindow:(id)sender { [self sendCommand:SC2KCommandOpenOrdinancesWindow]; }
- (void)openScenarioProgressWindow:(id)sender { [self sendCommand:SC2KCommandOpenScenarioProgressWindow]; }
- (void)dataMapCity:(id)sender { [self sendCommand:SC2KCommandDataMapCity]; }
- (void)dataMapPower:(id)sender { [self sendCommand:SC2KCommandDataMapPower]; }
- (void)dataMapWater:(id)sender { [self sendCommand:SC2KCommandDataMapWater]; }
- (void)dataMapTraffic:(id)sender { [self sendCommand:SC2KCommandDataMapTraffic]; }
- (void)dataMapLandValue:(id)sender { [self sendCommand:SC2KCommandDataMapLandValue]; }
- (void)dataMapPollution:(id)sender { [self sendCommand:SC2KCommandDataMapPollution]; }
- (void)dataMapCrime:(id)sender { [self sendCommand:SC2KCommandDataMapCrime]; }
- (void)dataMapPolice:(id)sender { [self sendCommand:SC2KCommandDataMapPolice]; }
- (void)dataMapFire:(id)sender { [self sendCommand:SC2KCommandDataMapFire]; }
- (void)dataMapDensity:(id)sender { [self sendCommand:SC2KCommandDataMapDensity]; }
- (void)dataMapGrowth:(id)sender { [self sendCommand:SC2KCommandDataMapGrowth]; }
- (void)dataMapHeight:(id)sender { [self sendCommand:SC2KCommandDataMapHeight]; }
- (void)toggleUnderground:(id)sender { [self sendCommand:SC2KCommandToggleUnderground]; }
- (void)toggleAutoBudget:(id)sender { [self sendCommand:SC2KCommandToggleAutoBudget]; }
- (void)toggleAutoGoto:(id)sender { [self sendCommand:SC2KCommandToggleAutoGoto]; }
- (void)toggleSoundEffects:(id)sender { [self sendCommand:SC2KCommandToggleSoundEffects]; }
- (void)toggleMusic:(id)sender { [self sendCommand:SC2KCommandToggleMusic]; }
- (void)appearanceSystem:(id)sender { [self sendCommand:SC2KCommandAppearanceSystem]; }
- (void)appearanceLight:(id)sender { [self sendCommand:SC2KCommandAppearanceLight]; }
- (void)appearanceDark:(id)sender { [self sendCommand:SC2KCommandAppearanceDark]; }
- (void)importOriginalAssets:(id)sender { [self sendCommand:SC2KCommandImportOriginalAssets]; }
- (void)chooseOriginalAssets:(id)sender { [self sendCommand:SC2KCommandChooseOriginalAssets]; }
- (void)chooseReplacementAssets:(id)sender { [self sendCommand:SC2KCommandChooseReplacementAssets]; }
- (void)preferOriginalGraphics:(id)sender { [self sendCommand:SC2KCommandPreferOriginalGraphics]; }
- (void)preferReplacementGraphics:(id)sender { [self sendCommand:SC2KCommandPreferReplacementGraphics]; }
- (void)nextGraphicsSource:(id)sender { [self sendCommand:SC2KCommandNextGraphicsSource]; }
- (void)nextEffectsSource:(id)sender { [self sendCommand:SC2KCommandNextEffectsSource]; }
- (void)nextMusicSource:(id)sender { [self sendCommand:SC2KCommandNextMusicSource]; }
- (void)toggleCityGameEffects:(id)sender { [self sendCommand:SC2KCommandToggleCityGameEffects]; }
- (void)toggleCityGameMusic:(id)sender { [self sendCommand:SC2KCommandToggleCityGameMusic]; }
- (void)toggleCityGameCityMusic:(id)sender { [self sendCommand:SC2KCommandToggleCityGameCityMusic]; }
- (void)toggleCityGameToolbar:(id)sender { [self sendCommand:SC2KCommandToggleCityGameToolbar]; }
- (void)toggleCityGameNewspaper:(id)sender { [self sendCommand:SC2KCommandToggleCityGameNewspaper]; }
- (void)toggleNoDisasters:(id)sender { [self sendCommand:SC2KCommandToggleNoDisasters]; }
- (void)startFire:(id)sender { [self sendCommand:SC2KCommandStartFire]; }
- (void)startFlood:(id)sender { [self sendCommand:SC2KCommandStartFlood]; }
- (void)startAirCrash:(id)sender { [self sendCommand:SC2KCommandStartAirCrash]; }
- (void)startTornado:(id)sender { [self sendCommand:SC2KCommandStartTornado]; }
- (void)startEarthquake:(id)sender { [self sendCommand:SC2KCommandStartEarthquake]; }
- (void)startMonster:(id)sender { [self sendCommand:SC2KCommandStartMonster]; }
- (void)startHurricane:(id)sender { [self sendCommand:SC2KCommandStartHurricane]; }
- (void)startRioters:(id)sender { [self sendCommand:SC2KCommandStartRioters]; }
- (void)openNewspaper:(id)sender { [self sendCommand:SC2KCommandOpenNewspaper]; }
- (void)openCourier:(id)sender { [self sendCommand:SC2KCommandOpenCourier]; }
- (void)openExtra:(id)sender { [self sendCommand:SC2KCommandOpenExtra]; }
- (void)openHelp:(id)sender { [self sendCommand:SC2KCommandOpenHelp]; }
- (void)toggleBalloonHelp:(id)sender { [self sendCommand:SC2KCommandToggleBalloonHelp]; }

@end

static SC2KMenuTarget *gTarget = nil;
static NSMenuItem *gAutoBudgetItem = nil;
static NSMenuItem *gAutoGotoItem = nil;
static NSMenuItem *gSoundEffectsItem = nil;
static NSMenuItem *gMusicItem = nil;
static NSMenuItem *gPreferOriginalGraphicsItem = nil;
static NSMenuItem *gPreferReplacementGraphicsItem = nil;
static NSMenuItem *gCityGameEffectsItem = nil;
static NSMenuItem *gCityGameMusicItem = nil;
static NSMenuItem *gCityGameCityMusicItem = nil;
static NSMenuItem *gCityGameToolbarItem = nil;
static NSMenuItem *gCityGameNewspaperItem = nil;
static NSMenuItem *gNoDisastersItem = nil;
static NSMenuItem *gNewspaperSubscriptionItem = nil;
static NSMenuItem *gNewspaperExtraItem = nil;
static NSMenuItem *gBalloonHelpItem = nil;
static NSMenuItem *gDataMapCityItem = nil;
static NSMenuItem *gDataMapPowerItem = nil;
static NSMenuItem *gDataMapWaterItem = nil;
static NSMenuItem *gDataMapTrafficItem = nil;
static NSMenuItem *gDataMapLandValueItem = nil;
static NSMenuItem *gDataMapPollutionItem = nil;
static NSMenuItem *gDataMapCrimeItem = nil;
static NSMenuItem *gDataMapPoliceItem = nil;
static NSMenuItem *gDataMapFireItem = nil;
static NSMenuItem *gDataMapDensityItem = nil;
static NSMenuItem *gDataMapGrowthItem = nil;
static NSMenuItem *gDataMapHeightItem = nil;
static NSMenuItem *gUndergroundItem = nil;
#define SC2K_DISASTER_MENU_ITEM_COUNT 8
static NSMenuItem *gDisasterItems[SC2K_DISASTER_MENU_ITEM_COUNT] = { nil };
static NSMenuItem *gSpeedMenuRoot = nil;
static NSMenuItem *gSpeedPauseItem = nil;
static NSMenuItem *gSpeedTurtleItem = nil;
static NSMenuItem *gSpeedLlamaItem = nil;
static NSMenuItem *gSpeedCheetahItem = nil;
static NSMenuItem *gSpeedSwallowItem = nil;

void sc2000_set_game_speed(int speed_command);
void sc2000_set_auto_budget(int enabled);
void sc2000_set_auto_goto(int enabled);
void sc2000_set_sound_effects(int enabled);
void sc2000_set_music(int enabled);
void sc2000_set_no_disasters(int enabled);
void sc2000_set_newspaper_subscription(int enabled);
void sc2000_set_newspaper_extra(int enabled);
void sc2000_set_balloon_help(int enabled);
void sc2000_set_graphics_preference(int prefer_replacement);
void sc2000_set_citygame_overrides(int effects, int music, int city_music, int toolbar);
void sc2000_set_appearance_preference(int preference);
void sc2000_set_data_map_mode(int mode_command);
void sc2000_set_underground_view(int enabled);

static NSMenuItem *SC2KMenuItem(NSString *title, SEL action, NSString *key, NSEventModifierFlags modifiers) {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title action:action keyEquivalent:key];
    item.target = gTarget;
    item.keyEquivalentModifierMask = modifiers;
    return item;
}

static NSMenuItem *SC2KDisasterMenuItem(NSUInteger index, NSString *title, SEL action) {
    NSMenuItem *item = SC2KMenuItem(title, action, @"", 0);
    if (index < SC2K_DISASTER_MENU_ITEM_COUNT) {
        gDisasterItems[index] = item;
    }
    return item;
}

static NSMenuItem *SC2KCheckedMenuItem(NSString *title, SEL action, NSString *key, NSEventModifierFlags modifiers) {
    NSMenuItem *item = SC2KMenuItem(title, action, key, modifiers);
    item.state = NSControlStateValueOn;
    return item;
}

void sc2000_install_menu(SC2KMenuCallback callback, SC2KDocumentOpenCallback documentOpenCallback) {
    @autoreleasepool {
        gCallback = callback;
        gDocumentOpenCallback = documentOpenCallback;
        if (gTarget == nil) {
            gTarget = [[SC2KMenuTarget alloc] init];
        }

        [NSApplication sharedApplication];
        [NSApp setDelegate:gTarget];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];

        NSEventModifierFlags command = NSEventModifierFlagCommand;
        NSEventModifierFlags commandShift = NSEventModifierFlagCommand | NSEventModifierFlagShift;
        NSString *appName = @"CityGame";

        NSMenu *mainMenu = [[NSMenu alloc] initWithTitle:@""];

        NSMenuItem *appRoot = [[NSMenuItem alloc] initWithTitle:@"" action:nil keyEquivalent:@""];
        NSMenu *appMenu = [[NSMenu alloc] initWithTitle:appName];
        [appMenu addItem:SC2KMenuItem([@"About " stringByAppendingString:appName], @selector(about:), @"", 0)];
        [appMenu addItem:[NSMenuItem separatorItem]];
        NSMenuItem *quitItem = [[NSMenuItem alloc] initWithTitle:[@"Quit " stringByAppendingString:appName]
                                                          action:@selector(terminate:)
                                                   keyEquivalent:@"q"];
        quitItem.target = NSApp;
        [appMenu addItem:quitItem];
        appRoot.submenu = appMenu;
        [mainMenu addItem:appRoot];

        NSMenuItem *fileRoot = [[NSMenuItem alloc] initWithTitle:@"File" action:nil keyEquivalent:@""];
        NSMenu *fileMenu = [[NSMenu alloc] initWithTitle:@"File"];
        [fileMenu addItem:SC2KMenuItem(@"Load Tile Set", @selector(loadTileSet:), @"", 0)];
        [fileMenu addItem:[NSMenuItem separatorItem]];
        [fileMenu addItem:SC2KMenuItem(@"Load City", @selector(openCity:), @"l", command)];
        [fileMenu addItem:SC2KMenuItem(@"New City", @selector(newCity:), @"n", command)];
        [fileMenu addItem:SC2KMenuItem(@"Edit New Map", @selector(toggleEditMap:), @"e", command)];
        [fileMenu addItem:SC2KMenuItem(@"Load Scenario", @selector(loadScenario:), @"z", command)];
        [fileMenu addItem:[NSMenuItem separatorItem]];
        [fileMenu addItem:SC2KMenuItem(@"Save City", @selector(saveCity:), @"s", command)];
        [fileMenu addItem:SC2KMenuItem(@"Save City As...", @selector(saveCityAs:), @"", 0)];
        [fileMenu addItem:[NSMenuItem separatorItem]];
        [fileMenu addItem:SC2KMenuItem(@"Return to Title Screen...", @selector(returnToTitle:), @"", 0)];
        [fileMenu addItem:[NSMenuItem separatorItem]];
        NSMenuItem *fileQuitItem = [[NSMenuItem alloc] initWithTitle:@"Quit" action:@selector(terminate:) keyEquivalent:@"q"];
        fileQuitItem.target = NSApp;
        fileQuitItem.keyEquivalentModifierMask = command;
        [fileMenu addItem:fileQuitItem];
        fileRoot.submenu = fileMenu;
        [mainMenu addItem:fileRoot];

        NSMenuItem *speedRoot = [[NSMenuItem alloc] initWithTitle:@"Speed" action:nil keyEquivalent:@""];
        NSMenu *speedMenu = [[NSMenu alloc] initWithTitle:@"Speed"];
        gSpeedPauseItem = SC2KMenuItem(@"Pause", @selector(speedPause:), @"p", command);
        gSpeedTurtleItem = SC2KMenuItem(@"Turtle", @selector(speedTurtle:), @"1", command);
        gSpeedLlamaItem = SC2KMenuItem(@"Llama", @selector(speedLlama:), @"2", command);
        gSpeedCheetahItem = SC2KMenuItem(@"Cheetah", @selector(speedCheetah:), @"3", command);
        gSpeedSwallowItem = SC2KMenuItem(@"African Swallow", @selector(speedSwallow:), @"4", command);
        [speedMenu addItem:gSpeedPauseItem];
        [speedMenu addItem:gSpeedTurtleItem];
        [speedMenu addItem:gSpeedLlamaItem];
        [speedMenu addItem:gSpeedCheetahItem];
        [speedMenu addItem:gSpeedSwallowItem];
        speedRoot.submenu = speedMenu;
        gSpeedMenuRoot = speedRoot;
        [mainMenu addItem:speedRoot];
        sc2000_set_game_speed(SC2KCommandSpeedLlama);

        NSMenuItem *optionsRoot = [[NSMenuItem alloc] initWithTitle:@"Options" action:nil keyEquivalent:@""];
        NSMenu *optionsMenu = [[NSMenu alloc] initWithTitle:@"Options"];
        gAutoBudgetItem = SC2KMenuItem(@"Auto-Budget", @selector(toggleAutoBudget:), @"", 0);
        gAutoGotoItem = SC2KMenuItem(@"Auto-Goto", @selector(toggleAutoGoto:), @"", 0);
        gSoundEffectsItem = SC2KMenuItem(@"Sound Effects", @selector(toggleSoundEffects:), @"", 0);
        gMusicItem = SC2KMenuItem(@"Music", @selector(toggleMusic:), @"", 0);
        [optionsMenu addItem:gAutoBudgetItem];
        [optionsMenu addItem:gAutoGotoItem];
        [optionsMenu addItem:gSoundEffectsItem];
        [optionsMenu addItem:gMusicItem];
        [optionsMenu addItem:[NSMenuItem separatorItem]];
        gAppearanceSystemItem = SC2KMenuItem(@"Appearance: Follow System", @selector(appearanceSystem:), @"", 0);
        gAppearanceLightItem = SC2KMenuItem(@"Appearance: Light", @selector(appearanceLight:), @"", 0);
        gAppearanceDarkItem = SC2KMenuItem(@"Appearance: Dark", @selector(appearanceDark:), @"", 0);
        [optionsMenu addItem:gAppearanceSystemItem];
        [optionsMenu addItem:gAppearanceLightItem];
        [optionsMenu addItem:gAppearanceDarkItem];
        optionsRoot.submenu = optionsMenu;
        [mainMenu addItem:optionsRoot];

        NSMenuItem *assetsRoot = [[NSMenuItem alloc] initWithTitle:@"Assets" action:nil keyEquivalent:@""];
        NSMenu *assetsMenu = [[NSMenu alloc] initWithTitle:@"Assets"];
        [assetsMenu addItem:SC2KMenuItem(@"Import from Game Files...", @selector(importOriginalAssets:), @"", 0)];
        [assetsMenu addItem:SC2KMenuItem(@"Choose Extracted Original Pack...", @selector(chooseOriginalAssets:), @"", 0)];
        [assetsMenu addItem:SC2KMenuItem(@"Choose Our Graphics Pack...", @selector(chooseReplacementAssets:), @"", 0)];
        [assetsMenu addItem:[NSMenuItem separatorItem]];
        [assetsMenu addItem:SC2KMenuItem(@"Next Graphics Edition", @selector(nextGraphicsSource:), @"", 0)];
        [assetsMenu addItem:SC2KMenuItem(@"Next Sound Edition", @selector(nextEffectsSource:), @"", 0)];
        [assetsMenu addItem:SC2KMenuItem(@"Next Music Edition", @selector(nextMusicSource:), @"", 0)];
        [assetsMenu addItem:[NSMenuItem separatorItem]];
        gPreferOriginalGraphicsItem = SC2KMenuItem(@"Prefer Original Graphics", @selector(preferOriginalGraphics:), @"", 0);
        gPreferReplacementGraphicsItem = SC2KMenuItem(@"Prefer High-Res CityGame Graphics", @selector(preferReplacementGraphics:), @"", 0);
        [assetsMenu addItem:gPreferOriginalGraphicsItem];
        [assetsMenu addItem:gPreferReplacementGraphicsItem];
        gCityGameEffectsItem = SC2KMenuItem(@"Use CityGame Sounds", @selector(toggleCityGameEffects:), @"", 0);
        gCityGameMusicItem = SC2KMenuItem(@"Use CityGame Title Music", @selector(toggleCityGameMusic:), @"", 0);
        gCityGameCityMusicItem = SC2KMenuItem(@"Use CityGame City Music", @selector(toggleCityGameCityMusic:), @"", 0);
        gCityGameToolbarItem = SC2KMenuItem(@"Use CityGame Toolbar Palette", @selector(toggleCityGameToolbar:), @"", 0);
        gCityGameNewspaperItem = SC2KMenuItem(@"Show CityGame Newspaper", @selector(toggleCityGameNewspaper:), @"", 0);
        [assetsMenu addItem:gCityGameEffectsItem];
        [assetsMenu addItem:gCityGameMusicItem];
        [assetsMenu addItem:gCityGameCityMusicItem];
        [assetsMenu addItem:gCityGameToolbarItem];
        [assetsMenu addItem:gCityGameNewspaperItem];
        [assetsMenu addItem:[NSMenuItem separatorItem]];
        [assetsMenu addItem:SC2KMenuItem(@"Reload Assets", @selector(reloadAssets:), @"r", commandShift)];
        assetsRoot.submenu = assetsMenu;
        [mainMenu addItem:assetsRoot];
        sc2000_set_graphics_preference(0);

        NSMenuItem *disastersRoot = [[NSMenuItem alloc] initWithTitle:@"Disasters" action:nil keyEquivalent:@""];
        NSMenu *disastersMenu = [[NSMenu alloc] initWithTitle:@"Disasters"];
        [disastersMenu addItem:SC2KDisasterMenuItem(0, @"Fire", @selector(startFire:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(1, @"Flood", @selector(startFlood:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(2, @"Air Crash", @selector(startAirCrash:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(3, @"Tornado", @selector(startTornado:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(4, @"Earthquake", @selector(startEarthquake:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(5, @"Monster", @selector(startMonster:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(6, @"Hurricane", @selector(startHurricane:))];
        [disastersMenu addItem:SC2KDisasterMenuItem(7, @"Rioters", @selector(startRioters:))];
        [disastersMenu addItem:[NSMenuItem separatorItem]];
        gNoDisastersItem = SC2KMenuItem(@"No Disasters", @selector(toggleNoDisasters:), @"", 0);
        [disastersMenu addItem:gNoDisastersItem];
        disastersRoot.submenu = disastersMenu;
        [mainMenu addItem:disastersRoot];

        NSMenuItem *viewRoot = [[NSMenuItem alloc] initWithTitle:@"View" action:nil keyEquivalent:@""];
        NSMenu *viewMenu = [[NSMenu alloc] initWithTitle:@"View"];
        gDataMapCityItem = SC2KMenuItem(@"City", @selector(dataMapCity:), @"", 0);
        gDataMapPowerItem = SC2KMenuItem(@"Power Supply", @selector(dataMapPower:), @"", 0);
        gDataMapWaterItem = SC2KMenuItem(@"Water Supply", @selector(dataMapWater:), @"", 0);
        gDataMapTrafficItem = SC2KMenuItem(@"Traffic", @selector(dataMapTraffic:), @"", 0);
        gDataMapLandValueItem = SC2KMenuItem(@"Land Value", @selector(dataMapLandValue:), @"", 0);
        gDataMapPollutionItem = SC2KMenuItem(@"Pollution", @selector(dataMapPollution:), @"", 0);
        gDataMapCrimeItem = SC2KMenuItem(@"Crime", @selector(dataMapCrime:), @"", 0);
        gDataMapPoliceItem = SC2KMenuItem(@"Police Power", @selector(dataMapPolice:), @"", 0);
        gDataMapFireItem = SC2KMenuItem(@"Fire Power", @selector(dataMapFire:), @"", 0);
        gDataMapDensityItem = SC2KMenuItem(@"Density", @selector(dataMapDensity:), @"", 0);
        gDataMapGrowthItem = SC2KMenuItem(@"Rate of Growth", @selector(dataMapGrowth:), @"", 0);
        gDataMapHeightItem = SC2KMenuItem(@"Heightmap", @selector(dataMapHeight:), @"", 0);
        gUndergroundItem = SC2KMenuItem(@"Underground", @selector(toggleUnderground:), @"", 0);
        [viewMenu addItem:gDataMapCityItem];
        [viewMenu addItem:gDataMapPowerItem];
        [viewMenu addItem:gDataMapWaterItem];
        [viewMenu addItem:gDataMapTrafficItem];
        [viewMenu addItem:gDataMapLandValueItem];
        [viewMenu addItem:gDataMapPollutionItem];
        [viewMenu addItem:gDataMapCrimeItem];
        [viewMenu addItem:gDataMapPoliceItem];
        [viewMenu addItem:gDataMapFireItem];
        [viewMenu addItem:gDataMapDensityItem];
        [viewMenu addItem:gDataMapGrowthItem];
        [viewMenu addItem:gDataMapHeightItem];
        [viewMenu addItem:gUndergroundItem];
        viewRoot.submenu = viewMenu;
        [mainMenu addItem:viewRoot];

        NSMenuItem *windowRoot = [[NSMenuItem alloc] initWithTitle:@"Windows" action:nil keyEquivalent:@""];
        NSMenu *windowMenu = [[NSMenu alloc] initWithTitle:@"Windows"];
        [windowMenu addItem:SC2KMenuItem(@"Map", @selector(openMapWindow:), @"m", command)];
        [windowMenu addItem:SC2KMenuItem(@"Budget", @selector(openBudgetWindow:), @"b", command)];
        [windowMenu addItem:SC2KMenuItem(@"Ordinances", @selector(openOrdinancesWindow:), @"o", command)];
        [windowMenu addItem:SC2KMenuItem(@"Population", @selector(openPopulationWindow:), @"c", command)];
        [windowMenu addItem:SC2KMenuItem(@"Industry", @selector(openIndustryWindow:), @"i", command)];
        [windowMenu addItem:SC2KMenuItem(@"Graphs", @selector(openGraphsWindow:), @"g", command)];
        [windowMenu addItem:SC2KMenuItem(@"Neighbors", @selector(openNeighborsWindow:), @"h", command)];
        windowRoot.submenu = windowMenu;
        [mainMenu addItem:windowRoot];

        NSMenuItem *newspaperRoot = [[NSMenuItem alloc] initWithTitle:@"Newspaper" action:nil keyEquivalent:@""];
        NSMenu *newspaperMenu = [[NSMenu alloc] initWithTitle:@"Newspaper"];
        gNewspaperSubscriptionItem = SC2KMenuItem(@"Subscription", @selector(openNewspaper:), @"", 0);
        [newspaperMenu addItem:gNewspaperSubscriptionItem];
        gNewspaperExtraItem = SC2KMenuItem(@"Extra!!!", @selector(openExtra:), @"", 0);
        [newspaperMenu addItem:gNewspaperExtraItem];
        [newspaperMenu addItem:[NSMenuItem separatorItem]];
        [newspaperMenu addItem:SC2KCheckedMenuItem(@"Courier", @selector(openCourier:), @"", 0)];
        newspaperRoot.submenu = newspaperMenu;
        [mainMenu addItem:newspaperRoot];

        NSMenuItem *helpRoot = [[NSMenuItem alloc] initWithTitle:@"Help" action:nil keyEquivalent:@""];
        NSMenu *helpMenu = [[NSMenu alloc] initWithTitle:@"Help"];
        [helpMenu addItem:SC2KMenuItem(@"About Balloon Help...", @selector(openHelp:), @"", 0)];
        gBalloonHelpItem = SC2KMenuItem(@"Show Balloons", @selector(toggleBalloonHelp:), @"", 0);
        [helpMenu addItem:gBalloonHelpItem];
        [helpMenu addItem:[NSMenuItem separatorItem]];
        [helpMenu addItem:SC2KMenuItem(@"Scenario Progress", @selector(openScenarioProgressWindow:), @"", 0)];
        helpRoot.submenu = helpMenu;
        [mainMenu addItem:helpRoot];

        sc2000_set_data_map_mode(SC2KCommandDataMapCity);

        NSMenuItem *debugRoot = [[NSMenuItem alloc] initWithTitle:@"Debug" action:nil keyEquivalent:@""];
        NSMenu *debugMenu = [[NSMenu alloc] initWithTitle:@"Debug"];
        [debugMenu addItem:SC2KMenuItem(@"Reload Assets", @selector(reloadAssets:), @"r", command)];
        // Mac 68k map keys: '+' zoom in, '-' zoom out. ('=' rotate is handled in-app so it
        // works consistently regardless of which window is focused — no menu accelerator.)
        [debugMenu addItem:SC2KMenuItem(@"Zoom In", @selector(zoomIn:), @"+", 0)];
        [debugMenu addItem:SC2KMenuItem(@"Zoom Out", @selector(zoomOut:), @"-", 0)];
        [debugMenu addItem:SC2KMenuItem(@"Actual Size", @selector(actualSize:), @"0", command)];
        [debugMenu addItem:[NSMenuItem separatorItem]];
        [debugMenu addItem:SC2KMenuItem(@"Center Map", @selector(centerMap:), @"c", commandShift)];
        [debugMenu addItem:SC2KMenuItem(@"Fit Whole Map", @selector(fitMap:), @"f", commandShift)];
        [debugMenu addItem:SC2KMenuItem(@"Toggle Grid", @selector(toggleGrid:), @"g", commandShift)];
        [debugMenu addItem:SC2KMenuItem(@"Toggle Zone Overlay", @selector(toggleZoneOverlay:), @"z", commandShift)];
        [debugMenu addItem:SC2KMenuItem(@"Toggle Traffic", @selector(toggleTraffic:), @"t", commandShift)];
        [debugMenu addItem:SC2KMenuItem(@"Inspect Tile", @selector(inspectTile:), @"i", commandShift)];
        [debugMenu addItem:SC2KMenuItem(@"Inspect Hover Tile", @selector(inspectHoverTile:), @"h", commandShift)];
        [debugMenu addItem:[NSMenuItem separatorItem]];
        [debugMenu addItem:SC2KMenuItem(@"Rotate Counter-Clockwise", @selector(rotateCcw:), @"", 0)];
        [debugMenu addItem:SC2KMenuItem(@"Rotate Clockwise", @selector(rotateCw:), @"]", command)];
        debugRoot.submenu = debugMenu;
        [mainMenu addItem:debugRoot];

        [NSApp setMainMenu:mainMenu];
    }
}

void sc2000_set_edit_map_mode(int edit_map) {
    @autoreleasepool {
        if (gSpeedMenuRoot != nil) {
            gSpeedMenuRoot.enabled = edit_map ? NO : YES;
        }
    }
}

void sc2000_set_game_speed(int speed_command) {
    @autoreleasepool {
        NSMenuItem *items[] = {
            gSpeedPauseItem,
            gSpeedTurtleItem,
            gSpeedLlamaItem,
            gSpeedCheetahItem,
            gSpeedSwallowItem,
        };
        int commands[] = {
            SC2KCommandSpeedPause,
            SC2KCommandSpeedTurtle,
            SC2KCommandSpeedLlama,
            SC2KCommandSpeedCheetah,
            SC2KCommandSpeedSwallow,
        };
        for (NSUInteger i = 0; i < sizeof(items) / sizeof(items[0]); i++) {
            if (items[i] != nil) {
                items[i].state = (commands[i] == speed_command)
                    ? NSControlStateValueOn
                    : NSControlStateValueOff;
            }
        }
    }
}

void sc2000_set_data_map_mode(int mode_command) {
    @autoreleasepool {
        NSMenuItem *items[] = {
            gDataMapCityItem,
            gDataMapPowerItem,
            gDataMapWaterItem,
            gDataMapTrafficItem,
            gDataMapLandValueItem,
            gDataMapPollutionItem,
            gDataMapCrimeItem,
            gDataMapPoliceItem,
            gDataMapFireItem,
            gDataMapDensityItem,
            gDataMapGrowthItem,
            gDataMapHeightItem,
        };
        int commands[] = {
            SC2KCommandDataMapCity,
            SC2KCommandDataMapPower,
            SC2KCommandDataMapWater,
            SC2KCommandDataMapTraffic,
            SC2KCommandDataMapLandValue,
            SC2KCommandDataMapPollution,
            SC2KCommandDataMapCrime,
            SC2KCommandDataMapPolice,
            SC2KCommandDataMapFire,
            SC2KCommandDataMapDensity,
            SC2KCommandDataMapGrowth,
            SC2KCommandDataMapHeight,
        };
        for (NSUInteger i = 0; i < sizeof(items) / sizeof(items[0]); i++) {
            if (items[i] != nil) {
                items[i].state = (commands[i] == mode_command)
                    ? NSControlStateValueOn
                    : NSControlStateValueOff;
            }
        }
    }
}

void sc2000_set_underground_view(int enabled) {
    @autoreleasepool {
        if (gUndergroundItem != nil) {
            gUndergroundItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_auto_budget(int enabled) {
    @autoreleasepool {
        if (gAutoBudgetItem != nil) {
            gAutoBudgetItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_auto_goto(int enabled) {
    @autoreleasepool {
        if (gAutoGotoItem != nil) {
            gAutoGotoItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_sound_effects(int enabled) {
    @autoreleasepool {
        if (gSoundEffectsItem != nil) {
            gSoundEffectsItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_music(int enabled) {
    @autoreleasepool {
        if (gMusicItem != nil) {
            gMusicItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_no_disasters(int enabled) {
    @autoreleasepool {
        if (gNoDisastersItem != nil) {
            gNoDisastersItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
        for (NSUInteger i = 0; i < SC2K_DISASTER_MENU_ITEM_COUNT; i++) {
            if (gDisasterItems[i] != nil) {
                gDisasterItems[i].enabled = !enabled;
            }
        }
    }
}

void sc2000_set_newspaper_subscription(int enabled) {
    @autoreleasepool {
        if (gNewspaperSubscriptionItem != nil) {
            gNewspaperSubscriptionItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_newspaper_extra(int enabled) {
    @autoreleasepool {
        if (gNewspaperExtraItem != nil) {
            gNewspaperExtraItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_balloon_help(int enabled) {
    @autoreleasepool {
        if (gBalloonHelpItem != nil) {
            gBalloonHelpItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_graphics_preference(int prefer_replacement) {
    @autoreleasepool {
        if (gPreferOriginalGraphicsItem != nil) {
            gPreferOriginalGraphicsItem.state = prefer_replacement
                ? NSControlStateValueOff
                : NSControlStateValueOn;
        }
        if (gPreferReplacementGraphicsItem != nil) {
            gPreferReplacementGraphicsItem.state = prefer_replacement
                ? NSControlStateValueOn
                : NSControlStateValueOff;
        }
    }
}

void sc2000_set_citygame_overrides(int effects, int music, int city_music, int toolbar) {
    @autoreleasepool {
        if (gCityGameEffectsItem != nil) {
            gCityGameEffectsItem.state = effects ? NSControlStateValueOn : NSControlStateValueOff;
        }
        if (gCityGameMusicItem != nil) {
            gCityGameMusicItem.state = music ? NSControlStateValueOn : NSControlStateValueOff;
        }
        if (gCityGameCityMusicItem != nil) {
            gCityGameCityMusicItem.state = city_music ? NSControlStateValueOn : NSControlStateValueOff;
        }
        if (gCityGameToolbarItem != nil) {
            gCityGameToolbarItem.state = toolbar ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_citygame_newspaper(int enabled) {
    @autoreleasepool {
        if (gCityGameNewspaperItem != nil) {
            gCityGameNewspaperItem.state = enabled ? NSControlStateValueOn : NSControlStateValueOff;
        }
    }
}

void sc2000_set_appearance_preference(int preference) {
    @autoreleasepool {
        if (gAppearanceSystemItem != nil) gAppearanceSystemItem.state = preference == 0 ? NSControlStateValueOn : NSControlStateValueOff;
        if (gAppearanceLightItem != nil) gAppearanceLightItem.state = preference == 1 ? NSControlStateValueOn : NSControlStateValueOff;
        if (gAppearanceDarkItem != nil) gAppearanceDarkItem.state = preference == 2 ? NSControlStateValueOn : NSControlStateValueOff;
    }
}

void sc2000_pump_events(void) {
    @autoreleasepool {
        for (;;) {
            NSEvent *event = [NSApp nextEventMatchingMask:NSEventMaskAny
                                                  untilDate:[NSDate distantPast]
                                                     inMode:NSDefaultRunLoopMode
                                                    dequeue:YES];
            if (event == nil) {
                break;
            }
            [NSApp sendEvent:event];
        }
    }
}

void sc2000_activate_app(void) {
    @autoreleasepool {
        [NSApp activateIgnoringOtherApps:YES];
    }
}

static NSWindow *SC2KWindowFromPointer(void *window_ptr) {
    if (window_ptr == NULL) {
        return nil;
    }
    return (__bridge NSWindow *)window_ptr;
}

void sc2000_configure_palette_window(void *palette_ptr, void *main_ptr) {
    @autoreleasepool {
        NSWindow *palette = SC2KWindowFromPointer(palette_ptr);
        NSWindow *main = SC2KWindowFromPointer(main_ptr);
        if (palette == nil) {
            return;
        }

        // CityGame tool palette: draggable title bar, no close/minimize controls.
        NSWindowStyleMask style = [palette styleMask];
        style &= ~(NSWindowStyleMaskClosable | NSWindowStyleMaskMiniaturizable);
        [palette setStyleMask:style];
        [palette setLevel:NSFloatingWindowLevel];
        [palette setCollectionBehavior:(NSWindowCollectionBehaviorMoveToActiveSpace
                                      | NSWindowCollectionBehaviorFullScreenAuxiliary)];
        [palette setHidesOnDeactivate:YES];
        [palette setExcludedFromWindowsMenu:YES];

        if (main != nil) {
            [main setLevel:NSNormalWindowLevel];
        }
    }
}

void sc2000_order_palette_above_main(void *palette_ptr, void *main_ptr) {
    @autoreleasepool {
        NSWindow *palette = SC2KWindowFromPointer(palette_ptr);
        NSWindow *main = SC2KWindowFromPointer(main_ptr);
        if (palette == nil || main == nil) {
            return;
        }

        // Stack the palette above the map without making it key (menus stay on the main window).
        [palette orderWindow:NSWindowAbove relativeTo:main.windowNumber];
    }
}

void sc2000_focus_main_window(void *main_ptr) {
    @autoreleasepool {
        NSWindow *main = SC2KWindowFromPointer(main_ptr);
        if (main == nil) {
            return;
        }
        [NSApp activateIgnoringOtherApps:YES];
        [main makeKeyAndOrderFront:nil];
        [main displayIfNeeded];
        [main.contentView displayIfNeeded];
    }
}

void sc2000_present_windows(void *main_ptr, void *palette_ptr) {
    @autoreleasepool {
        NSWindow *main = SC2KWindowFromPointer(main_ptr);
        NSWindow *palette = SC2KWindowFromPointer(palette_ptr);
        if (main == nil) {
            return;
        }

        [NSApp activateIgnoringOtherApps:YES];
        [main makeKeyAndOrderFront:nil];
        [main displayIfNeeded];
        [main.contentView displayIfNeeded];
        if (palette != nil) {
            [palette orderWindow:NSWindowAbove relativeTo:main.windowNumber];
            [palette displayIfNeeded];
            [palette.contentView displayIfNeeded];
        }
        [NSApp updateWindows];
    }
}
