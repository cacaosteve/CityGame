#import <Foundation/Foundation.h>

typedef void (*SC2KDocumentOpenCallback)(const char *);

BOOL sc2k_dispatch_document_open_paths(
    NSArray<NSString *> *filenames,
    SC2KDocumentOpenCallback callback);
