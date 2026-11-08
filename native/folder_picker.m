// Isolated AppKit folder picker. No SDL, renderer, importer, or game state.
#import <Cocoa/Cocoa.h>
#include <stdio.h>
#include <string.h>

@interface CityGameFolderPickerDelegate : NSObject <NSApplicationDelegate>
@property(nonatomic, copy) NSString *initialDirectory;
@end

@implementation CityGameFolderPickerDelegate
- (void)applicationDidFinishLaunching:(NSNotification *)notification {
    // Present after NSApplication's regular event loop has started, as in a
    // native document app. Keep this process independent of SDL's NSApplication.
    dispatch_async(dispatch_get_main_queue(), ^{
        [NSApp activateIgnoringOtherApps:YES];
        NSOpenPanel *panel = [NSOpenPanel openPanel];
        panel.title = @"CityGame";
        panel.message = @"Choose a folder for CityGame.";
        panel.prompt = @"Choose";
        panel.canChooseFiles = NO;
        panel.canChooseDirectories = YES;
        panel.allowsMultipleSelection = NO;
        panel.directoryURL = [NSURL fileURLWithPath:self.initialDirectory ?: NSHomeDirectory()
                                       isDirectory:YES];
        if ([panel runModal] == NSModalResponseOK) {
            const char *path = panel.URL.fileSystemRepresentation;
            if (path == NULL || fwrite(path, 1, strlen(path), stdout) != strlen(path) || fflush(stdout) != 0) {
                fprintf(stderr, "Could not return the selected folder.\n");
                exit(1);
            }
        }
        // Success returns the path verbatim with no newline. Cancellation
        // returns an empty stream. No selected folder is opened or imported.
        [NSApp terminate:nil];
    });
}
@end

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        // A helper panel should not add a second Dock icon or app menu.
        app.activationPolicy = NSApplicationActivationPolicyAccessory;
        CityGameFolderPickerDelegate *delegate = [CityGameFolderPickerDelegate new];
        if (argc > 1) {
            delegate.initialDirectory = [[NSFileManager defaultManager]
                stringWithFileSystemRepresentation:argv[1] length:strlen(argv[1])];
        }
        app.delegate = delegate;
        [app run];
    }
    return 0;
}
