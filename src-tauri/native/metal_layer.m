#import <Cocoa/Cocoa.h>
#import <OpenGL/OpenGL.h>
#import <OpenGL/gl3.h>
#import <QuartzCore/QuartzCore.h>
#include "metal_layer.h"

// ---------------------------------------------------------------------------
// MpvOpenGLLayer — CAOpenGLLayer subclass for mpv render API
// ---------------------------------------------------------------------------

// Declared in Rust — renders mpv frame to the given FBO
extern void blowup_render_mpv_frame(int fbo, int width, int height);

@interface MpvOpenGLLayer : CAOpenGLLayer
@property (nonatomic, assign) CGLContextObj cglContext;
@property (nonatomic, assign) CGLPixelFormatObj cglPixelFormat;
@end

@implementation MpvOpenGLLayer

- (CGLPixelFormatObj)copyCGLPixelFormatForDisplayMask:(uint32_t)mask {
    CGLPixelFormatAttribute attrs[] = {
        kCGLPFAOpenGLProfile, (CGLPixelFormatAttribute)kCGLOGLPVersion_3_2_Core,
        kCGLPFADoubleBuffer,
        kCGLPFAAllowOfflineRenderers,
        kCGLPFAAccelerated,
        0
    };
    CGLPixelFormatObj pix = NULL;
    GLint npix = 0;
    CGLChoosePixelFormat(attrs, &pix, &npix);
    self.cglPixelFormat = pix;
    NSLog(@"blowup: copyCGLPixelFormat — npix=%d", npix);
    return pix;
}

- (CGLContextObj)copyCGLContextForPixelFormat:(CGLPixelFormatObj)pf {
    CGLContextObj ctx = NULL;
    CGLCreateContext(pf, NULL, &ctx);
    self.cglContext = ctx;
    NSLog(@"blowup: copyCGLContext — ctx=%p", ctx);
    return ctx;
}

- (BOOL)canDrawInCGLContext:(CGLContextObj)ctx
                pixelFormat:(CGLPixelFormatObj)pf
               forLayerTime:(CFTimeInterval)t
                displayTime:(const CVTimeStamp *)ts {
    // Always ready — mpv decides if there's a new frame
    return YES;
}

- (void)drawInCGLContext:(CGLContextObj)ctx
             pixelFormat:(CGLPixelFormatObj)pf
            forLayerTime:(CFTimeInterval)t
             displayTime:(const CVTimeStamp *)ts {
    CGLSetCurrentContext(ctx);

    // Get the layer's FBO and viewport (NOT necessarily FBO 0)
    GLint fbo = 0;
    glGetIntegerv(GL_FRAMEBUFFER_BINDING, &fbo);
    GLint viewport[4];
    glGetIntegerv(GL_VIEWPORT, viewport);
    int w = viewport[2];
    int h = viewport[3];

    // Call into Rust — mpv renders to the layer's actual FBO
    blowup_render_mpv_frame(fbo, w, h);

    // Super calls glFlush /
    [super drawInCGLContext:ctx pixelFormat:pf forLayerTime:t displayTime:ts];
}

@end

// ---------------------------------------------------------------------------
// BlowupGLView — NSView hosting the MpvOpenGLLayer
// ---------------------------------------------------------------------------

@interface BlowupGLView : NSView
@property (nonatomic, strong) MpvOpenGLLayer *glLayer;
@end

@implementation BlowupGLView

- (instancetype)initWithFrame:(NSRect)frame {
    self = [super initWithFrame:frame];
    if (self) {
        MpvOpenGLLayer *layer = [[MpvOpenGLLayer alloc] init];
        layer.asynchronous = NO;  // We drive rendering via blowup_request_render
        layer.contentsScale = [[NSScreen mainScreen] backingScaleFactor];
        layer.autoresizingMask = kCALayerWidthSizable | kCALayerHeightSizable;
        layer.needsDisplayOnBoundsChange = YES;

        [self setLayer:layer];
        [self setWantsLayer:YES];
        self.glLayer = layer;
    }
    return self;
}

@end

// ---------------------------------------------------------------------------
// C bridge
// ---------------------------------------------------------------------------

void* blowup_create_gl_view(double width, double height) {
    NSRect frame = NSMakeRect(0, 0, width, height);
    BlowupGLView *view = [[BlowupGLView alloc] initWithFrame:frame];
    view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    NSLog(@"blowup: GL view created %.0fx%.0f", width, height);
    return (__bridge_retained void *)view;
}

int blowup_attach_to_window(void *ns_window_ptr, void *view_ptr) {
    if (!ns_window_ptr || !view_ptr) return -1;

    NSWindow *window = (__bridge NSWindow *)ns_window_ptr;
    NSView *glView = (__bridge NSView *)view_ptr;
    NSView *contentView = window.contentView;
    if (!contentView) {
        NSLog(@"blowup: no contentView");
        return -1;
    }

    // Find WKWebView
    NSView *webView = nil;
    for (NSView *subview in contentView.subviews) {
        if ([subview isKindOfClass:NSClassFromString(@"WKWebView")]) {
            webView = subview;
            break;
        }
    }

    glView.frame = contentView.bounds;

    if (webView) {
        [contentView addSubview:glView positioned:NSWindowBelow relativeTo:webView];

        // Make WKWebView transparent
        @try {
            [webView setValue:@(NO) forKey:@"drawsBackground"];
        } @catch (NSException *e) {
            NSLog(@"blowup: failed to set drawsBackground: %@", e);
        }
        if (webView.layer) {
            webView.layer.backgroundColor = CGColorGetConstantColor(kCGColorClear);
            webView.layer.opaque = NO;
        }
        for (NSView *sub in webView.subviews) {
            if (sub.layer) {
                sub.layer.backgroundColor = CGColorGetConstantColor(kCGColorClear);
                sub.layer.opaque = NO;
            }
            for (NSView *sub2 in sub.subviews) {
                if (sub2.layer) {
                    sub2.layer.backgroundColor = CGColorGetConstantColor(kCGColorClear);
                    sub2.layer.opaque = NO;
                }
            }
        }
    } else {
        [contentView addSubview:glView positioned:NSWindowBelow relativeTo:nil];
    }

    window.backgroundColor = [NSColor clearColor];
    window.opaque = NO;
    window.appearance = [NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    contentView.wantsLayer = YES;
    contentView.layer.backgroundColor = CGColorGetConstantColor(kCGColorClear);
    contentView.layer.opaque = NO;

    // Trigger initial draw
    [((BlowupGLView *)glView).glLayer setNeedsDisplay];

    NSLog(@"blowup: GL view attached, webView: %@, subviews: %lu",
          webView ? @"YES" : @"NO",
          (unsigned long)contentView.subviews.count);
    return 0;
}

void* blowup_get_cgl_context(void *view_ptr) {
    if (!view_ptr) return NULL;
    BlowupGLView *view = (__bridge BlowupGLView *)view_ptr;
    // Force layer to create context if not yet
    if (!view.glLayer.cglContext) {
        [view.glLayer setNeedsDisplay];
    }
    return view.glLayer.cglContext;
}

// OpenGL proc address lookup — uses Apple's CGL
static void* gl_get_proc_address(void *ctx, const char *name) {
    (void)ctx;
    CFBundleRef bundle = CFBundleGetBundleWithIdentifier(CFSTR("com.apple.opengl"));
    if (!bundle) return NULL;
    CFStringRef cfName = CFStringCreateWithCString(kCFAllocatorDefault, name, kCFStringEncodingASCII);
    void *addr = CFBundleGetFunctionPointerForName(bundle, cfName);
    CFRelease(cfName);
    return addr;
}

void* blowup_get_gl_proc_address(void) {
    return (void *)gl_get_proc_address;
}

void blowup_make_gl_context_current(void *view_ptr) {
    if (!view_ptr) return;
    BlowupGLView *view = (__bridge BlowupGLView *)view_ptr;
    MpvOpenGLLayer *layer = view.glLayer;

    // Force layer to create pixel format + context if not yet done
    if (!layer.cglContext) {
        [layer setNeedsDisplay];
        [CATransaction flush];
    }

    if (layer.cglContext) {
        CGLSetCurrentContext(layer.cglContext);
        NSLog(@"blowup: CGL context made current: %p", layer.cglContext);
    } else {
        NSLog(@"blowup: WARNING — no CGL context available");
    }
}

void blowup_request_render(void *view_ptr) {
    if (!view_ptr) return;
    BlowupGLView *view = (__bridge BlowupGLView *)view_ptr;
    // Schedule redraw on main thread
    dispatch_async(dispatch_get_main_queue(), ^{
        [view.glLayer setNeedsDisplay];
    });
}

void blowup_remove_view(void *view_ptr) {
    if (!view_ptr) return;
    NSView *view = (__bridge_transfer NSView *)view_ptr;
    [view removeFromSuperview];
    NSLog(@"blowup: view removed");
}
