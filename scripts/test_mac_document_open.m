#import <Foundation/Foundation.h>
#import "../native/mac_document_open.h"
#import <string.h>

static NSMutableArray<NSString *> *openedPaths;

static void record_opened_path(const char *path) {
    NSString *value = [NSString stringWithUTF8String:path];
    [openedPaths addObject:value];
}

int main(void) {
    @autoreleasepool {
        openedPaths = [NSMutableArray array];
        NSArray<NSString *> *paths = @[@"/tmp/first.sc2", @"/tmp/café.sc2", @"/tmp/third.sc2"];
        NSCAssert(sc2k_dispatch_document_open_paths(paths, record_opened_path), @"batch should dispatch");
        NSMutableArray<NSString *> *expectedPaths = [NSMutableArray array];
        for (NSString *path in paths) {
            [expectedPaths addObject:[NSString stringWithUTF8String:[path fileSystemRepresentation]]];
        }
        NSCAssert([openedPaths isEqualToArray:expectedPaths], @"all paths should dispatch once in input order");

        openedPaths = [NSMutableArray array];
        NSArray<NSString *> *invalidBatch = @[@"/tmp/valid.sc2", @""];
        NSCAssert(!sc2k_dispatch_document_open_paths(invalidBatch, record_opened_path), @"empty path should reject batch");
        NSCAssert(openedPaths.count == 0, @"invalid batch must not partially dispatch");
        NSCAssert(!sc2k_dispatch_document_open_paths(@[], record_opened_path), @"empty batch should reject");
        NSCAssert(!sc2k_dispatch_document_open_paths(paths, NULL), @"missing callback should reject");
    }
    return 0;
}
