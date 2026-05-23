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
#define ACTION_WINDOW_WIDTH 560
#define ACTION_GAP 10.0
#define BUTTON_GAP 6.0
#define BUTTON_HEIGHT 26.0
#define BUTTON_HORIZONTAL_PADDING 12.0
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
static StatusActionSet g_currentActions = STATUS_ACTIONS_NONE;
static char *g_currentText = NULL;
static StatusOverlayActionCallback g_actionCallback = NULL;
static gboolean g_initialized = FALSE;
static gboolean g_visible = FALSE;
static pthread_mutex_t g_mutex = PTHREAD_MUTEX_INITIALIZER;

typedef struct {
    StatusOverlayAction action;
    const char *label;
    double x;
    double y;
    double width;
    double height;
} ActionButton;

// Forward declarations
static gboolean on_draw(GtkWidget *widget, cairo_t *cr, gpointer data);
static gboolean on_button_release(GtkWidget *widget, GdkEventButton *event, gpointer data);
static int calculate_window_width(void);
static void calculate_window_position(int window_width, int *x, int *y);
static void update_input_shape(int window_width);

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

static int action_count(StatusActionSet actions) {
    if (actions == STATUS_ACTIONS_RETRY) return 1;
    if (actions == STATUS_ACTIONS_RETRY_CANCEL) return 2;
    return 0;
}

static double button_width(cairo_t *cr, const char *label) {
    cairo_text_extents_t extents;
    cairo_text_extents(cr, label, &extents);
    double width = extents.width + BUTTON_HORIZONTAL_PADDING * 2.0;
    return width < 46.0 ? 46.0 : width;
}

static int action_buttons(
    cairo_t *cr,
    int window_width,
    StatusActionSet actions,
    ActionButton buttons[2]
) {
    int count = action_count(actions);
    if (count == 0) return 0;

    const char *retry_label = "重试";
    const char *cancel_label = "取消";
    buttons[0].action = STATUS_ACTION_RETRY;
    buttons[0].label = retry_label;
    buttons[0].width = button_width(cr, retry_label);
    buttons[0].height = BUTTON_HEIGHT;
    if (count == 2) {
        buttons[1].action = STATUS_ACTION_CANCEL;
        buttons[1].label = cancel_label;
        buttons[1].width = button_width(cr, cancel_label);
        buttons[1].height = BUTTON_HEIGHT;
    }

    double total_width = 0.0;
    for (int i = 0; i < count; i++) {
        total_width += buttons[i].width;
    }
    total_width += BUTTON_GAP * (count - 1);

    double cursor_x = window_width - HORIZONTAL_PADDING - total_width;
    for (int i = 0; i < count; i++) {
        buttons[i].x = cursor_x;
        buttons[i].y = (WINDOW_HEIGHT - BUTTON_HEIGHT) / 2.0;
        cursor_x += buttons[i].width + BUTTON_GAP;
    }
    return count;
}

// Draw callback
static gboolean on_draw(GtkWidget *widget, cairo_t *cr, gpointer data) {
    (void)widget;
    (void)data;
    
    pthread_mutex_lock(&g_mutex);
    StatusType status = g_currentStatus;
    StatusActionSet actions = g_currentActions;
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

    ActionButton buttons[2];
    int button_count = action_buttons(cr, window_width, actions, buttons);
    double button_start_x = button_count > 0 ? buttons[0].x : window_width;
    
    // Calculate text position (centered)
    cairo_text_extents_t extents;
    cairo_text_extents(cr, text, &extents);
    double text_x = HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP - extents.x_bearing;
    double text_y = (WINDOW_HEIGHT - extents.height) / 2.0 - extents.y_bearing;
    double text_clip_width = button_count > 0
        ? button_start_x - ACTION_GAP - (HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP)
        : window_width - HORIZONTAL_PADDING * 2.0 - DOT_SIZE - CONTENT_GAP;
    if (text_clip_width < 0.0) {
        text_clip_width = 0.0;
    }
    
    cairo_save(cr);
    cairo_rectangle(
        cr,
        HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP,
        0,
        text_clip_width,
        WINDOW_HEIGHT
    );
    cairo_clip(cr);
    cairo_move_to(cr, text_x, text_y);
    cairo_show_text(cr, text);
    cairo_restore(cr);

    for (int i = 0; i < button_count; i++) {
        ActionButton button = buttons[i];
        draw_rounded_rect(cr, button.x, button.y, button.width, button.height, BUTTON_HEIGHT / 2.0);
        cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 0.16);
        cairo_fill(cr);
        draw_rounded_rect(cr, button.x + 0.5, button.y + 0.5, button.width - 1.0, button.height - 1.0, BUTTON_HEIGHT / 2.0);
        cairo_set_source_rgba(cr, 1.0, 1.0, 1.0, 0.22);
        cairo_set_line_width(cr, 1.0);
        cairo_stroke(cr);

        cairo_text_extents_t label_extents;
        cairo_text_extents(cr, button.label, &label_extents);
        cairo_set_source_rgb(cr, 1.0, 1.0, 1.0);
        cairo_move_to(
            cr,
            button.x + (button.width - label_extents.width) / 2.0 - label_extents.x_bearing,
            button.y + (button.height - label_extents.height) / 2.0 - label_extents.y_bearing
        );
        cairo_show_text(cr, button.label);
    }
    
    free(text);
    return FALSE;
}

static int calculate_window_width(void) {
    pthread_mutex_lock(&g_mutex);
    char *text = g_currentText ? strdup(g_currentText) : strdup(" ");
    StatusActionSet actions = g_currentActions;
    pthread_mutex_unlock(&g_mutex);

    cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    cairo_t *cr = cairo_create(surface);
    cairo_select_font_face(cr, "Sans", CAIRO_FONT_SLANT_NORMAL, CAIRO_FONT_WEIGHT_BOLD);
    cairo_set_font_size(cr, 13.0);

    cairo_text_extents_t extents;
    cairo_text_extents(cr, text, &extents);
    ActionButton buttons[2];
    int button_count = action_buttons(cr, ACTION_WINDOW_WIDTH, actions, buttons);
    double button_group_width = 0.0;
    if (button_count > 0) {
        for (int i = 0; i < button_count; i++) {
            button_group_width += buttons[i].width;
        }
        button_group_width += BUTTON_GAP * (button_count - 1);
    }

    cairo_destroy(cr);
    cairo_surface_destroy(surface);
    free(text);

    double action_width = button_group_width > 0.0 ? ACTION_GAP + button_group_width : 0.0;
    int ideal_width = (int)(HORIZONTAL_PADDING * 2.0 + DOT_SIZE + CONTENT_GAP + extents.width + action_width + 0.999);
    if (ideal_width < 1) {
        return 1;
    }
    int max_width = actions == STATUS_ACTIONS_NONE ? MAX_WINDOW_WIDTH : ACTION_WINDOW_WIDTH;
    return ideal_width > max_width ? max_width : ideal_width;
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

static void update_input_shape(int window_width) {
    if (!g_window) {
        return;
    }
    GdkWindow *gdk_window = gtk_widget_get_window(g_window);
    if (!gdk_window) {
        return;
    }

    pthread_mutex_lock(&g_mutex);
    StatusActionSet actions = g_currentActions;
    pthread_mutex_unlock(&g_mutex);

    cairo_region_t *region = NULL;
    if (actions == STATUS_ACTIONS_NONE) {
        region = cairo_region_create();
    } else {
        cairo_rectangle_int_t rect = {0, 0, window_width, WINDOW_HEIGHT};
        region = cairo_region_create_rectangle(&rect);
    }
    gdk_window_input_shape_combine_region(gdk_window, region, 0, 0);
    cairo_region_destroy(region);
}

static gboolean on_button_release(GtkWidget *widget, GdkEventButton *event, gpointer data) {
    (void)widget;
    (void)data;

    pthread_mutex_lock(&g_mutex);
    StatusActionSet actions = g_currentActions;
    StatusOverlayActionCallback callback = g_actionCallback;
    pthread_mutex_unlock(&g_mutex);
    if (actions == STATUS_ACTIONS_NONE || !callback) {
        return FALSE;
    }

    cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    cairo_t *cr = cairo_create(surface);
    cairo_select_font_face(cr, "Sans", CAIRO_FONT_SLANT_NORMAL, CAIRO_FONT_WEIGHT_BOLD);
    cairo_set_font_size(cr, 13.0);
    int window_width = calculate_window_width();
    ActionButton buttons[2];
    int count = action_buttons(cr, window_width, actions, buttons);
    cairo_destroy(cr);
    cairo_surface_destroy(surface);

    for (int i = 0; i < count; i++) {
        ActionButton button = buttons[i];
        if (event->x >= button.x && event->x <= button.x + button.width &&
            event->y >= button.y && event->y <= button.y + button.height) {
            callback(button.action);
            return TRUE;
        }
    }
    return FALSE;
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
        update_input_shape(window_width);
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
    gtk_widget_add_events(g_window, GDK_BUTTON_RELEASE_MASK);
    g_signal_connect(g_window, "button-release-event", G_CALLBACK(on_button_release), NULL);
    
    // Set initial position
    int x, y;
    calculate_window_position(MAX_WINDOW_WIDTH, &x, &y);
    gtk_window_move(GTK_WINDOW(g_window), x, y);
    
    // Realize window but don't show yet
    gtk_widget_realize(g_window);
    
    update_input_shape(MAX_WINDOW_WIDTH);
    
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

void status_overlay_show_actions(StatusType status, const char* text, StatusActionSet actions) {
    if (!g_initialized || !g_window) return;
    
    pthread_mutex_lock(&g_mutex);
    g_currentStatus = status;
    g_currentActions = actions;
    free(g_currentText);
    g_currentText = text ? strdup(text) : strdup("");
    pthread_mutex_unlock(&g_mutex);
    
    g_idle_add(update_window_callback, NULL);
    g_idle_add(show_window_callback, NULL);
}

void status_overlay_show(StatusType status, const char* text) {
    status_overlay_show_actions(status, text, STATUS_ACTIONS_NONE);
}

void status_overlay_set_action_callback(StatusOverlayActionCallback callback) {
    pthread_mutex_lock(&g_mutex);
    g_actionCallback = callback;
    pthread_mutex_unlock(&g_mutex);
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
    g_currentActions = STATUS_ACTIONS_NONE;
    g_actionCallback = NULL;
    pthread_mutex_unlock(&g_mutex);
    
    g_initialized = FALSE;
    g_visible = FALSE;
}
