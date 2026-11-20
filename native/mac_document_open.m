#import "mac_document_open.h"

BOOL sc2k_dispatch_document_open_paths(
    NSArray<NSString *> *filenames,
    SC2KDocumentOpenCallback callback) {
    if (callback == NULL || filenames.count == 0) {
        return NO;
    }

    for (id item in filenames) {
        if (![item isKindOfClass:[NSString class]]) {
            return NO;
        }
        NSString *path = (NSString *)item;
        if (path.length == 0 || [path fileSystemRepresentation] == NULL) {
            return NO;
        }
    }

    for (NSString *path in filenames) {
        callback([path fileSystemRepresentation]);
    }
    return YES;
}
