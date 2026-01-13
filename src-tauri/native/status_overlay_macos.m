/**
 * Native status overlay for macOS using Cocoa/AppKit.
 * Creates a floating, transparent, click-through window at the bottom center of the screen.
 */

#import <Cocoa/Cocoa.h>
#include "status_overlay.h"
#include <pthread.h>

// Window configuration (matching Windows implementation)
static const CGFloat WINDOW_WIDTH = 200.0;
static const CGFloat WINDOW_HEIGHT = 36.0;
static const CGFloat CORNER_RADIUS = 18.0;
static const CGFloat BOTTOM_MARGIN = 48.0;
static const CGFloat WINDOW_ALPHA = 0.9;  // 230/255

// Colors for different status types (RGB)
static const CGFloat STATUS_COLORS[][3] = {
    {239.0/255.0, 68.0/255.0, 68.0/255.0},    // Recording - Red #ef4444
    {59.0/255.0, 130.0/255.0, 246.0/255.0},   // Transcribing - Blue #3b82f6
    {16.0/255.0, 185.0/255.0, 129.0/255.0},   // Completed - Green #10b981
    {245.0/255.0, 158.0/255.0, 11.0/255.0},   // Error - Orange #f59e0b
};

// Global state
static NSWindow *g_window = nil;
static NSView *g_contentView = nil;
static StatusType g_currentStatus = STATUS_RECORDING;
static NSString *g_currentText = @"";
static BOOL g_initialized = NO;
static BOOL g_visible = NO;
static pthread_mutex_t g_mutex = PTHREAD_MUTEX_INITIALIZER;

// Custom view for drawing the status overlay
@interface StatusOverlayView : NSView
@end

@implementation StatusOverlayView

- (BOOL)isOpaque {
    return NO;
}

- (void)drawRect:(NSRect)dirtyRect {
    [super drawRect:dirtyRect];
    
    pthread_mutex_lock(&g_mutex);
    StatusType status = g_currentStatus;
    NSString *text = [g_currentText copy];
    pthread_mutex_unlock(&g_mutex);
    
    // Clear background
    [[NSColor clearColor] setFill];
    NSRectFill(dirtyRect);
    
    // Draw rounded rectangle background
    NSRect bounds = self.bounds;
    NSBezierPath *path = [NSBezierPath bezierPathWithRoundedRect:bounds
                                                         xRadius:CORNER_RADIUS
                                                         yRadius:CORNER_RADIUS];
    
    NSColor *bgColor = [NSColor colorWithRed:STATUS_COLORS[status][0]
                                       green:STATUS_COLORS[status][1]
                                        blue:STATUS_COLORS[status][2]
                                       alpha:1.0];
    [bgColor setFill];
    [path fill];
    
    // Draw text
    NSMutableParagraphStyle *paragraphStyle = [[NSMutableParagraphStyle alloc] init];
    paragraphStyle.alignment = NSTextAlignmentCenter;
    
    NSDictionary *attributes = @{
        NSFontAttributeName: [NSFont systemFontOfSize:13.0 weight:NSFontWeightBold],
        NSForegroundColorAttributeName: [NSColor whiteColor],
        NSParagraphStyleAttributeName: paragraphStyle
    };
    
    NSSize textSize = [text sizeWithAttributes:attributes];
    CGFloat textY = (bounds.size.height - textSize.height) / 2.0;
    NSRect textRect = NSMakeRect(0, textY, bounds.size.width, textSize.height);
    
    [text drawInRect:textRect withAttributes:attributes];
}

@end

// Calculate window position (bottom center of screen)
static NSPoint CalculateWindowPosition(void) {
    NSScreen *screen = [NSScreen mainScreen];
    NSRect visibleFrame = screen.visibleFrame;
    
    CGFloat x = visibleFrame.origin.x + (visibleFrame.size.width - WINDOW_WIDTH) / 2.0;
    CGFloat y = visibleFrame.origin.y + BOTTOM_MARGIN;
    
    return NSMakePoint(x, y);
}

// Update window content on main thread
static void UpdateWindowOnMainThread(void) {
    if (!g_window || !g_contentView) return;
    
    dispatch_async(dispatch_get_main_queue(), ^{
        [g_contentView setNeedsDisplay:YES];
    });
}

// Show window on main thread
static void ShowWindowOnMainThread(void) {
    if (!g_window) return;
    
    dispatch_async(dispatch_get_main_queue(), ^{
        NSPoint pos = CalculateWindowPosition();
        [g_window setFrameOrigin:pos];
        [g_window orderFrontRegardless];
        g_visible = YES;
    });
}

// Hide window on main thread
static void HideWindowOnMainThread(void) {
    if (!g_window) return;
    
    dispatch_async(dispatch_get_main_queue(), ^{
        [g_window orderOut:nil];
        g_visible = NO;
    });
}

// Initialize window on main thread
static void InitWindowOnMainThread(void) {
    NSPoint pos = CalculateWindowPosition();
    NSRect frame = NSMakeRect(pos.x, pos.y, WINDOW_WIDTH, WINDOW_HEIGHT);
    
    g_window = [[NSWindow alloc] initWithContentRect:frame
                                           styleMask:NSWindowStyleMaskBorderless
                                             backing:NSBackingStoreBuffered
                                               defer:NO];
    
    // Configure window properties
    g_window.level = NSFloatingWindowLevel;
    g_window.backgroundColor = [NSColor clearColor];
    g_window.opaque = NO;
    g_window.hasShadow = NO;
    g_window.ignoresMouseEvents = YES;
    g_window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                   NSWindowCollectionBehaviorStationary |
                                   NSWindowCollectionBehaviorIgnoresCycle;
    g_window.alphaValue = WINDOW_ALPHA;
    
    // Create and set content view
    g_contentView = [[StatusOverlayView alloc] initWithFrame:NSMakeRect(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT)];
    g_window.contentView = g_contentView;
}

// Public API implementations

int status_overlay_init(void) {
    if (g_initialized) return 0;
    
    // Ensure NSApplication is initialized
    [NSApplication sharedApplication];
    
    if ([NSThread isMainThread]) {
        InitWindowOnMainThread();
    } else {
        dispatch_sync(dispatch_get_main_queue(), ^{
            InitWindowOnMainThread();
        });
    }
    
    g_initialized = (g_window != nil);
    return g_initialized ? 0 : -1;
}

void status_overlay_show(StatusType status, const char* text) {
    if (!g_initialized || !g_window) return;
    
    pthread_mutex_lock(&g_mutex);
    g_currentStatus = status;
    g_currentText = text ? [NSString stringWithUTF8String:text] : @"";
    pthread_mutex_unlock(&g_mutex);
    
    UpdateWindowOnMainThread();
    
    if (!g_visible) {
        ShowWindowOnMainThread();
    }
}

void status_overlay_hide(void) {
    if (!g_initialized || !g_window) return;
    
    if (g_visible) {
        HideWindowOnMainThread();
    }
}

void status_overlay_cleanup(void) {
    if (!g_initialized) return;
    
    dispatch_async(dispatch_get_main_queue(), ^{
        if (g_window) {
            [g_window close];
            g_window = nil;
        }
        g_contentView = nil;
    });
    
    g_initialized = NO;
    g_visible = NO;
}
