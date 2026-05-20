/**
 * Native status overlay for Linux using GTK3 + Cairo.
 * Creates a floating, transparent, click-through window at the bottom center of the screen.
 */

#include <gtk/gtk.h>
#include <gdk/gdk.h>
#include <cairo.h>
#include <pthread.h>
#include <string.h>
#include <stdlib.h>
#include "status_overlay.h"

// Window configuration, kept visually aligned with the macOS Swift overlay.
#define MAX_WINDOW_WIDTH 420
#define WINDOW_HEIGHT 40
#define CORNER_RADIUS 20.0
#define BOTTOM_MARGIN 48
#define DOT_SIZE 8.0
#define HORIZONTAL_PADDING 14.0
#define CONTENT_GAP 8.0
#define WINDOW_ALPHA 1.0

// Colors for different status types (RGB normalized), matching the Swift status dot.
static const double STATUS_COLORS[][3] = {
    {239.0/255.0, 68.0/255.0, 68.0/255.0},    // Recording - Red #ef4444
    {59.0/255.0, 130.0/255.0, 246.0/255.0},   // Transcribing - Blue #3b82f6
    {16.0/255.0, 185.0/255.0, 129.0/255.0},   // Completed - Green #10b981
    {245.0/255.0, 158.0/255.0, 11.0/255.0},   // Error - Orange #f59e0b
};
#define BACKGROUND_R 0.08
#define BACKGROUND_G 0.08
#define BACKGROUND_B 0.08
#define BACKGROUND_A 0.88
#define BORDER_A 0.12

// Global state
static GtkWidget *g_window = NULL;
static StatusType g_currentStatus = STATUS_RECORDING;
static char *g_currentText = NULL;
static gboolean g_initialized = FALSE;
static gboolean g_visible = FALSE;
static pthread_mutex_t g_mutex = PTHREAD_MUTEX_INITIALIZER;

// Forward declarations
static gboolean on_draw(GtkWidget *widget, cairo_t *cr, gpointer data);
static int calculate_window_width(void);
static void calculate_window_position(int window_width, int *x, int *y);

// Draw rounded rectangle path
static void draw_rounded_rect(cairo_t *cr, double x, double y, double width, double height, double radius) {
    double degrees = G_PI / 180.0;
    
    cairo_new_sub_path(cr);
    cairo_arc(cr, x + width - radius, y + radius, radius, -90 * degrees, 0 * degrees);
    cairo_arc(cr, x + width - radius, y + height - radius, radius, 0 * degrees, 90 * degrees);
    cairo_arc(cr, x + radius, y + height - radius, radius, 90 * degrees, 180 * degrees);
    cairo_arc(cr, x + radius, y + radius, radius, 180 * degrees, 270 * degrees);
    cairo_close_path(cr);
}

// Draw callback
static gboolean on_draw(GtkWidget *widget, cairo_t *cr, gpointer data) {
    (void)widget;
    (void)data;
    
    pthread_mutex_lock(&g_mutex);
    StatusType status = g_currentStatus;
    char *text = g_currentText ? strdup(g_currentText) : strdup("");
    pthread_mutex_unlock(&g_mutex);
    
    // Clear background (transparent)
    cairo_set_source_rgba(cr, 0, 0, 0, 0);
    cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
    cairo_paint(cr);
    cairo_set_operator(cr, CAIRO_OPERATOR_OVER);
    
    // Draw rounded rectangle background
    int window_width = calculate_window_width();
    draw_rounded_rect(cr, 0, 0, window_width, WINDOW_HEIGHT, CORNER_RADIUS);
    cairo_set_source_rgba(cr, BACKGROUND_R, BACKGROUND_G, BACKGROUND_B, BACKGROUND_A);
    cairo_fill(cr);

    draw_rounded_rect(cr, 0.5, 0.5, window_width - 1.0, WINDOW_HEIGHT - 1.0, CORNER_RADIUS);
    cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, BORDER_A);
    cairo_set_line_width(cr, 1.0);
    cairo_stroke(cr);

    cairo_set_source_rgb(cr, STATUS_COLORS[status][0], STATUS_COLORS[status][1], STATUS_COLORS[status][2]);
    cairo_arc(
        cr,
        HORIZONTAL_PADDING + DOT_SIZE / 2.0,
        WINDOW_HEIGHT / 2.0,
        DOT_SIZE / 2.0,
        0,
        2.0 * G_PI
    );
    cairo_fill(cr);
    
    // Draw text
    cairo_select_font_face(cr, "Sans", CAIRO_FONT_SLANT_NORMAL, CAIRO_FONT_WEIGHT_BOLD);
    cairo_set_font_size(cr, 13.0);
    cairo_set_source_rgb(cr, 1.0, 1.0, 1.0);
    
    // Calculate text position (centered)
    cairo_text_extents_t extents;
    cairo_text_extents(cr, text, &extents);
    double text_x = HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP - extents.x_bearing;
    double text_y = (WINDOW_HEIGHT - extents.height) / 2.0 - extents.y_bearing;
    
    cairo_save(cr);
    cairo_rectangle(
        cr,
        HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP,
        0,
        window_width - HORIZONTAL_PADDING * 2.0 - DOT_SIZE - CONTENT_GAP,
        WINDOW_HEIGHT
    );
    cairo_clip(cr);
    cairo_move_to(cr, text_x, text_y);
    cairo_show_text(cr, text);
    cairo_restore(cr);
    
    free(text);
    return FALSE;
}

static int calculate_window_width(void) {
    pthread_mutex_lock(&g_mutex);
    char *text = g_currentText ? strdup(g_currentText) : strdup(" ");
    pthread_mutex_unlock(&g_mutex);

    cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    cairo_t *cr = cairo_create(surface);
    cairo_select_font_face(cr, "Sans", CAIRO_FONT_SLANT_NORMAL, CAIRO_FONT_WEIGHT_BOLD);
    cairo_set_font_size(cr, 13.0);

    cairo_text_extents_t extents;
    cairo_text_extents(cr, text, &extents);

    cairo_destroy(cr);
    cairo_surface_destroy(surface);
    free(text);

    int ideal_width = (int)(HORIZONTAL_PADDING * 2.0 + DOT_SIZE + CONTENT_GAP + extents.width + 0.999);
    if (ideal_width < 1) {
        return 1;
    }
    return ideal_width > MAX_WINDOW_WIDTH ? MAX_WINDOW_WIDTH : ideal_width;
}

// Calculate window position (bottom center of screen)
static void calculate_window_position(int window_width, int *x, int *y) {
    GdkDisplay *display = gdk_display_get_default();
    GdkMonitor *monitor = gdk_display_get_primary_monitor(display);
    if (!monitor) {
        monitor = gdk_display_get_monitor(display, 0);
    }
    
    GdkRectangle workarea;
    gdk_monitor_get_workarea(monitor, &workarea);
    
    *x = workarea.x + (workarea.width - window_width) / 2;
    *y = workarea.y + workarea.height - WINDOW_HEIGHT - BOTTOM_MARGIN;
}

// Enable RGBA visual for transparency
static void setup_visual(GtkWidget *widget) {
    GdkScreen *screen = gtk_widget_get_screen(widget);
    GdkVisual *visual = gdk_screen_get_rgba_visual(screen);
    
    if (visual) {
        gtk_widget_set_visual(widget, visual);
    }
}

// Update callback for main thread
static gboolean update_window_callback(gpointer data) {
    (void)data;
    if (g_window) {
        gtk_widget_queue_draw(g_window);
    }
    return G_SOURCE_REMOVE;
}

// Show callback for main thread
static gboolean show_window_callback(gpointer data) {
    (void)data;
    if (g_window) {
        int x, y;
        int window_width = calculate_window_width();
        gtk_window_set_default_size(GTK_WINDOW(g_window), window_width, WINDOW_HEIGHT);
        gtk_window_resize(GTK_WINDOW(g_window), window_width, WINDOW_HEIGHT);
        gtk_window_set_keep_above(GTK_WINDOW(g_window), TRUE);
        calculate_window_position(window_width, &x, &y);
        gtk_window_move(GTK_WINDOW(g_window), x, y);
        gtk_widget_show_all(g_window);
        gtk_window_present(GTK_WINDOW(g_window));
        g_visible = TRUE;
    }
    return G_SOURCE_REMOVE;
}

// Hide callback for main thread
static gboolean hide_window_callback(gpointer data) {
    (void)data;
    if (g_window) {
        gtk_widget_hide(g_window);
        g_visible = FALSE;
    }
    return G_SOURCE_REMOVE;
}

// Initialize callback for main thread
static gboolean init_window_callback(gpointer data) {
    gboolean *result = (gboolean *)data;
    
    // Create popup window
    g_window = gtk_window_new(GTK_WINDOW_POPUP);
    gtk_window_set_default_size(GTK_WINDOW(g_window), MAX_WINDOW_WIDTH, WINDOW_HEIGHT);
    gtk_window_set_resizable(GTK_WINDOW(g_window), FALSE);
    gtk_window_set_decorated(GTK_WINDOW(g_window), FALSE);
    gtk_window_set_skip_taskbar_hint(GTK_WINDOW(g_window), TRUE);
    gtk_window_set_skip_pager_hint(GTK_WINDOW(g_window), TRUE);
    gtk_window_set_keep_above(GTK_WINDOW(g_window), TRUE);
    gtk_widget_set_app_paintable(g_window, TRUE);
    
    // Enable transparency
    setup_visual(g_window);
    
    // Set window opacity
    gtk_widget_set_opacity(g_window, WINDOW_ALPHA);
    
    // Connect draw signal
    g_signal_connect(g_window, "draw", G_CALLBACK(on_draw), NULL);
    
    // Set initial position
    int x, y;
    calculate_window_position(MAX_WINDOW_WIDTH, &x, &y);
    gtk_window_move(GTK_WINDOW(g_window), x, y);
    
    // Realize window but don't show yet
    gtk_widget_realize(g_window);
    
    // Make window click-through (input pass-through)
    GdkWindow *gdk_window = gtk_widget_get_window(g_window);
    if (gdk_window) {
        cairo_region_t *region = cairo_region_create();
        gdk_window_input_shape_combine_region(gdk_window, region, 0, 0);
        cairo_region_destroy(region);
    }
    
    *result = (g_window != NULL);
    return G_SOURCE_REMOVE;
}

// Cleanup callback for main thread
static gboolean cleanup_window_callback(gpointer data) {
    (void)data;
    if (g_window) {
        gtk_widget_destroy(g_window);
        g_window = NULL;
    }
    return G_SOURCE_REMOVE;
}

// Public API implementations

int status_overlay_init(void) {
    if (g_initialized) return 0;
    
    // Initialize GTK if needed
    if (!gtk_init_check(NULL, NULL)) {
        return -1;
    }
    
    gboolean result = FALSE;
    
    // Create window on main thread
    if (g_main_context_is_owner(g_main_context_default())) {
        init_window_callback(&result);
    } else {
        g_idle_add(init_window_callback, &result);
        // Wait a bit for initialization
        g_usleep(100000);  // 100ms
        result = (g_window != NULL);
    }
    
    g_initialized = result;
    return result ? 0 : -1;
}

void status_overlay_show(StatusType status, const char* text) {
    if (!g_initialized || !g_window) return;
    
    pthread_mutex_lock(&g_mutex);
    g_currentStatus = status;
    free(g_currentText);
    g_currentText = text ? strdup(text) : strdup("");
    pthread_mutex_unlock(&g_mutex);
    
    g_idle_add(update_window_callback, NULL);
    g_idle_add(show_window_callback, NULL);
}

void status_overlay_hide(void) {
    if (!g_initialized || !g_window) return;
    
    if (g_visible) {
        g_idle_add(hide_window_callback, NULL);
    }
}

void status_overlay_cleanup(void) {
    if (!g_initialized) return;
    
    g_idle_add(cleanup_window_callback, NULL);
    
    pthread_mutex_lock(&g_mutex);
    free(g_currentText);
    g_currentText = NULL;
    pthread_mutex_unlock(&g_mutex);
    
    g_initialized = FALSE;
    g_visible = FALSE;
}
