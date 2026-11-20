#import <AVFoundation/AVFoundation.h>
#include <stdbool.h>

static AVMIDIPlayer *citygameTitlePlayer;

bool citygame_start_title_midi(const char *path) {
    @autoreleasepool {
        if (path == NULL) return false;
        NSString *file = [NSString stringWithUTF8String:path];
        if (file == nil) return false;
        NSURL *url = [NSURL fileURLWithPath:file];
        NSError *error = nil;
        AVMIDIPlayer *player = [[AVMIDIPlayer alloc] initWithContentsOfURL:url
                                                              soundBankURL:nil
                                                                     error:&error];
        if (player == nil) {
            NSLog(@"CityGame could not load title MIDI: %@", error);
            return false;
        }
        [citygameTitlePlayer stop];
        citygameTitlePlayer = player;
        [citygameTitlePlayer prepareToPlay];
        [citygameTitlePlayer play:nil];
        return citygameTitlePlayer.isPlaying;
    }
}

void citygame_update_title_midi(void) {
    @autoreleasepool {
        if (citygameTitlePlayer != nil && !citygameTitlePlayer.isPlaying &&
            citygameTitlePlayer.currentPosition >= citygameTitlePlayer.duration - 0.5) {
            citygameTitlePlayer.currentPosition = 0;
            [citygameTitlePlayer play:nil];
        }
    }
}

void citygame_stop_title_midi(void) {
    @autoreleasepool {
        [citygameTitlePlayer stop];
        citygameTitlePlayer = nil;
    }
}

bool citygame_midi_position_is_active(bool playing, double position, double duration) {
    return playing && (duration <= 0 || position < duration);
}

bool citygame_title_midi_is_playing(void) {
    @autoreleasepool {
        if (citygameTitlePlayer == nil || !citygameTitlePlayer.isPlaying) return false;
        if (!citygame_midi_position_is_active(true, citygameTitlePlayer.currentPosition,
                                               citygameTitlePlayer.duration)) {
            [citygameTitlePlayer stop];
            return false;
        }
        return true;
    }
}
