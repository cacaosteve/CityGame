// UIKit owns document-provider access. Rust keeps persistent local working copies.
#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <string.h>

const char *citygame_ios_documents_directory(void) {
    static NSString *directory;
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        directory = [[[NSFileManager defaultManager] URLsForDirectory:NSDocumentDirectory inDomains:NSUserDomainMask] firstObject].path;
    });
    return directory.fileSystemRepresentation;
}

typedef void (*CityGameDocumentCallback)(void *, const char *, const char *);
typedef void (*CityGameExternalDocumentCallback)(const char *, const char *);
static CityGameExternalDocumentCallback externalDocumentCallback;

@interface CityGameIncomingDocument : NSObject
@property(nonatomic, strong) NSURL *url;
@property(nonatomic) BOOL scoped;
@end
@implementation CityGameIncomingDocument
- (void)dealloc { if (self.scoped) [self.url stopAccessingSecurityScopedResource]; }
@end
static NSMutableArray<CityGameIncomingDocument *> *pendingDocuments;

static void CityGameDeliverDocument(CityGameIncomingDocument *document) {
    NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
    NSError *error = nil;
    __block BOOL delivered = NO;
    [coordinator coordinateReadingItemAtURL:document.url options:0 error:&error byAccessor:^(NSURL *url) {
        delivered = YES;
        externalDocumentCallback(url.fileSystemRepresentation, NULL);
    }];
    if (!delivered) externalDocumentCallback(NULL, (error.localizedDescription ?: @"The city could not be opened from Files.").UTF8String);
}

void citygame_ios_set_document_open_callback(CityGameExternalDocumentCallback callback) {
    externalDocumentCallback = callback;
    dispatch_async(dispatch_get_main_queue(), ^{
        NSArray *documents = [pendingDocuments copy];
        [pendingDocuments removeAllObjects];
        for (CityGameIncomingDocument *document in documents) CityGameDeliverDocument(document);
    });
}

// SDL 3's UIKit scene delegate supplies this subclass-selection hook. Keep its
// lifecycle, but retain the actual NSURL grant instead of reducing it to a path
// in an SDL DropFile event before the provider copy has been read.
@interface SDLUIKitSceneDelegate : NSObject <UIApplicationDelegate, UIWindowSceneDelegate>
+ (NSString *)getSceneDelegateClassName;
- (void)handleURL:(NSURL *)url;
@end
@interface CityGameSceneDelegate : SDLUIKitSceneDelegate
@end
@implementation CityGameSceneDelegate
- (UISceneConfiguration *)application:(UIApplication *)application configurationForConnectingSceneSession:(UISceneSession *)session options:(UISceneConnectionOptions *)options {
    UISceneConfiguration *config = [super application:application configurationForConnectingSceneSession:session options:options];
    config.delegateClass = [CityGameSceneDelegate class];
    return config;
}
- (void)handleURL:(NSURL *)url {
    if (!url.isFileURL) { [super handleURL:url]; return; }
    CityGameIncomingDocument *document = [CityGameIncomingDocument new];
    document.url = url;
    document.scoped = [url startAccessingSecurityScopedResource];
    if (externalDocumentCallback) {
        CityGameDeliverDocument(document);
    } else {
        if (!pendingDocuments) pendingDocuments = [NSMutableArray new];
        [pendingDocuments addObject:document];
    }
}
@end
@implementation SDLUIKitSceneDelegate (CityGameDocumentSupport)
+ (NSString *)getSceneDelegateClassName { return @"CityGameSceneDelegate"; }
@end

@interface CityGameDocumentPicker : NSObject <UIDocumentPickerDelegate, UIAdaptivePresentationControllerDelegate>
@property(nonatomic) CityGameDocumentCallback callback;
@property(nonatomic) void *context;
@property(nonatomic) BOOL exporting;
@property(nonatomic) BOOL finished;
@property(nonatomic, strong) UIDocumentPickerViewController *picker;
- (void)finishURL:(NSURL *)url error:(NSString *)error;
@end

static CityGameDocumentPicker *activePicker;

@implementation CityGameDocumentPicker
- (void)finishURL:(NSURL *)url error:(NSString *)error {
    if (self.finished) return;
    self.finished = YES;
    void (^complete)(void) = ^{
        if (url != nil && !self.exporting) {
            BOOL scoped = [url startAccessingSecurityScopedResource];
            NSFileCoordinator *coordinator = [[NSFileCoordinator alloc] initWithFilePresenter:nil];
            NSError *coordinationError = nil;
            __block BOOL delivered = NO;
            [coordinator coordinateReadingItemAtURL:url options:0 error:&coordinationError byAccessor:^(NSURL *readURL) {
                delivered = YES;
                self.callback(self.context, readURL.fileSystemRepresentation, NULL);
            }];
            if (!delivered) {
                NSString *message = coordinationError.localizedDescription ?: @"The selected file could not be read.";
                self.callback(self.context, NULL, message.UTF8String);
            }
            if (scoped) [url stopAccessingSecurityScopedResource];
        } else {
            self.callback(self.context, url.fileSystemRepresentation, error.UTF8String);
        }
        self.picker.delegate = nil;
        self.picker = nil;
        activePicker = nil;
    };
    if (self.picker.presentingViewController != nil) {
        [self.picker dismissViewControllerAnimated:YES completion:complete];
    } else {
        complete();
    }
}
- (void)documentPicker:(UIDocumentPickerViewController *)controller didPickDocumentsAtURLs:(NSArray<NSURL *> *)urls {
    [self finishURL:urls.firstObject error:nil];
}
- (void)documentPickerWasCancelled:(UIDocumentPickerViewController *)controller {
    [self finishURL:nil error:nil];
}
- (void)presentationControllerDidDismiss:(UIPresentationController *)presentationController {
    [self finishURL:nil error:nil];
}
@end

// Strings are copied before returning; the callback's C strings live through the call.
void citygame_ios_pick_document(void *windowPointer, const char *initialPath,
                               const char *exportPath, void *context,
                               CityGameDocumentCallback callback) {
    UIWindow *window = (__bridge UIWindow *)windowPointer;
    NSString *initial = initialPath ? [[NSFileManager defaultManager] stringWithFileSystemRepresentation:initialPath length:strlen(initialPath)] : nil;
    NSString *source = exportPath ? [[NSFileManager defaultManager] stringWithFileSystemRepresentation:exportPath length:strlen(exportPath)] : nil;
    dispatch_async(dispatch_get_main_queue(), ^{
        if (activePicker != nil) {
            callback(context, NULL, "A document picker is already open.");
            return;
        }
        CityGameDocumentPicker *owner = [CityGameDocumentPicker new];
        owner.callback = callback;
        owner.context = context;
        owner.exporting = source != nil;
        activePicker = owner;
        UIViewController *presenter = window.rootViewController;
        while (presenter.presentedViewController && !presenter.presentedViewController.isBeingDismissed) {
            presenter = presenter.presentedViewController;
        }
        if (presenter == nil || presenter.view.window == nil) {
            [owner finishURL:nil error:@"The game window is not ready to show Files."];
            return;
        }
        UIDocumentPickerViewController *picker = source != nil
            ? [[UIDocumentPickerViewController alloc] initForExportingURLs:@[[NSURL fileURLWithPath:source]] asCopy:YES]
            : [[UIDocumentPickerViewController alloc] initForOpeningContentTypes:@[UTTypeData] asCopy:YES];
        owner.picker = picker;
        picker.delegate = owner;
        picker.allowsMultipleSelection = NO;
        picker.shouldShowFileExtensions = YES;
        picker.modalPresentationStyle = UIModalPresentationFormSheet;
        if (initial != nil) picker.directoryURL = [NSURL fileURLWithPath:initial isDirectory:YES];
        [presenter presentViewController:picker animated:YES completion:^{
            picker.presentationController.delegate = owner;
        }];
    });
}

void citygame_ios_document_error(void *windowPointer, const char *message) {
    UIWindow *window = (__bridge UIWindow *)windowPointer;
    NSString *text = [NSString stringWithUTF8String:message] ?: @"The file operation failed.";
    dispatch_async(dispatch_get_main_queue(), ^{
        UIViewController *presenter = window.rootViewController;
        while (presenter.presentedViewController) presenter = presenter.presentedViewController;
        UIAlertController *alert = [UIAlertController alertControllerWithTitle:@"CityGame Files" message:text preferredStyle:UIAlertControllerStyleAlert];
        [alert addAction:[UIAlertAction actionWithTitle:@"OK" style:UIAlertActionStyleDefault handler:nil]];
        [presenter presentViewController:alert animated:YES completion:nil];
    });
}
