#ifndef STATUS_OVERLAY_H
#define STATUS_OVERLAY_H

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    STATUS_RECORDING = 0,
    STATUS_TRANSCRIBING = 1,
    STATUS_COMPLETED = 2,
    STATUS_ERROR = 3
} StatusType;

/**
 * Initialize the status overlay window.
 * Call once at application startup.
 * Returns 0 on success, non-zero on failure.
 */
int status_overlay_init(void);

/**
 * Show the status overlay with the given status type and text.
 * The window will automatically appear at the bottom center of the screen.
 */
void status_overlay_show(StatusType status, const char* text);

/**
 * Hide the status overlay window.
 */
void status_overlay_hide(void);

/**
 * Cleanup resources used by the status overlay.
 * Call once at application exit.
 */
void status_overlay_cleanup(void);

#ifdef __cplusplus
}
#endif

#endif /* STATUS_OVERLAY_H */
