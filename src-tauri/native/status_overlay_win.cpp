// Note: Do NOT define WIN32_LEAN_AND_MEAN before windows.h
// GDI+ requires COM headers (IStream, etc.) that LEAN_AND_MEAN excludes
#define UNICODE
#define _UNICODE

#include <windows.h>
#include <objidl.h>    // Required for IStream (used by GDI+)
#include <gdiplus.h>
#include <string>
#include <thread>
#include <mutex>
#include <condition_variable>
#include <atomic>
#include <cmath>

#pragma comment(lib, "gdiplus.lib")

#include "status_overlay.h"

namespace {

// Window configuration, kept visually aligned with the macOS Swift overlay.
constexpr int MAX_WINDOW_WIDTH = 420;
constexpr int WINDOW_HEIGHT = 40;
constexpr int CORNER_RADIUS = 20;
constexpr int BOTTOM_MARGIN = 48;
constexpr int DOT_SIZE = 8;
constexpr int HORIZONTAL_PADDING = 14;
constexpr int CONTENT_GAP = 8;
constexpr int ACTION_WINDOW_WIDTH = 560;
constexpr int ACTION_GAP = 10;
constexpr int BUTTON_GAP = 6;
constexpr int BUTTON_HEIGHT = 26;
constexpr int BUTTON_HORIZONTAL_PADDING = 12;
constexpr BYTE WINDOW_ALPHA = 255;

// Colors for different status types (ARGB), matching the Swift status dot.
const Gdiplus::Color STATUS_COLORS[] = {
    Gdiplus::Color(255, 239, 68, 68),   // Recording - Red
    Gdiplus::Color(255, 59, 130, 246),  // Transcribing - Blue
    Gdiplus::Color(255, 16, 185, 129),  // Completed - Green
    Gdiplus::Color(255, 245, 158, 11),  // Error - Orange
};
const Gdiplus::Color BACKGROUND_COLOR(224, 20, 20, 20);
const Gdiplus::Color BORDER_COLOR(31, 255, 255, 255);
const Gdiplus::Color TEXT_COLOR(255, 255, 255, 255);

// Global state
HWND g_hwnd = nullptr;
ULONG_PTR g_gdiplusToken = 0;
std::thread g_messageThread;
std::mutex g_mutex;
std::condition_variable g_cv;
std::atomic<bool> g_initialized{false};
std::atomic<bool> g_shouldExit{false};
std::atomic<bool> g_visible{false};

StatusType g_currentStatus = STATUS_RECORDING;
StatusActionSet g_currentActions = STATUS_ACTIONS_NONE;
std::wstring g_currentText;
StatusOverlayActionCallback g_actionCallback = nullptr;

const wchar_t* WINDOW_CLASS_NAME = L"VTTStatusOverlay";

struct ActionButton {
    StatusOverlayAction action;
    const wchar_t* label;
    Gdiplus::RectF rect;
};

int BuildActionButtons(
    Gdiplus::Graphics& graphics,
    const Gdiplus::Font& font,
    StatusActionSet actions,
    int windowWidth,
    ActionButton buttons[2]
);

// Convert UTF-8 to wide string
std::wstring Utf8ToWide(const char* utf8) {
    if (!utf8 || !*utf8) return L"";
    int len = MultiByteToWideChar(CP_UTF8, 0, utf8, -1, nullptr, 0);
    if (len <= 0) return L"";
    std::wstring result(len - 1, L'\0');
    MultiByteToWideChar(CP_UTF8, 0, utf8, -1, &result[0], len);
    return result;
}

// Get screen work area (excluding taskbar)
RECT GetWorkArea() {
    RECT workArea;
    SystemParametersInfo(SPI_GETWORKAREA, 0, &workArea, 0);
    return workArea;
}

int CalculateWindowWidth(HDC hdc) {
    std::wstring text;
    StatusActionSet actions = STATUS_ACTIONS_NONE;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        text = g_currentText.empty() ? L" " : g_currentText;
        actions = g_currentActions;
    }

    Gdiplus::Graphics graphics(hdc);
    Gdiplus::FontFamily fontFamily(L"Segoe UI");
    Gdiplus::Font font(&fontFamily, 13, Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
    Gdiplus::Font buttonFont(&fontFamily, 12, Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
    Gdiplus::RectF layoutRect(0.0f, 0.0f, static_cast<Gdiplus::REAL>(MAX_WINDOW_WIDTH), static_cast<Gdiplus::REAL>(WINDOW_HEIGHT));
    Gdiplus::RectF bounds;
    graphics.MeasureString(text.c_str(), -1, &font, layoutRect, &bounds);
    ActionButton buttons[2];
    int buttonCount = BuildActionButtons(graphics, buttonFont, actions, ACTION_WINDOW_WIDTH, buttons);
    Gdiplus::REAL buttonGroupWidth = 0.0f;
    for (int i = 0; i < buttonCount; ++i) {
        buttonGroupWidth += buttons[i].rect.Width;
    }
    if (buttonCount > 0) {
        buttonGroupWidth += static_cast<Gdiplus::REAL>(BUTTON_GAP * (buttonCount - 1));
    }

    int textWidth = static_cast<int>(std::ceil(bounds.Width));
    int actionWidth = buttonCount > 0 ? ACTION_GAP + static_cast<int>(std::ceil(buttonGroupWidth)) : 0;
    int idealWidth = HORIZONTAL_PADDING * 2 + DOT_SIZE + CONTENT_GAP + textWidth + actionWidth;
    int maxWidth = actions == STATUS_ACTIONS_NONE ? MAX_WINDOW_WIDTH : ACTION_WINDOW_WIDTH;
    return min(maxWidth, max(1, idealWidth));
}

// Calculate window position (bottom center)
POINT CalculateWindowPosition(int windowWidth) {
    RECT workArea = GetWorkArea();
    int screenWidth = workArea.right - workArea.left;
    int x = workArea.left + (screenWidth - windowWidth) / 2;
    int y = workArea.bottom - WINDOW_HEIGHT - BOTTOM_MARGIN;
    return {x, y};
}

int ActionCount(StatusActionSet actions) {
    if (actions == STATUS_ACTIONS_RETRY) return 1;
    if (actions == STATUS_ACTIONS_RETRY_CANCEL) return 2;
    return 0;
}

Gdiplus::REAL ButtonWidth(
    Gdiplus::Graphics& graphics,
    const Gdiplus::Font& font,
    const wchar_t* label
) {
    Gdiplus::RectF bounds;
    Gdiplus::RectF layoutRect(0.0f, 0.0f, 120.0f, static_cast<Gdiplus::REAL>(BUTTON_HEIGHT));
    graphics.MeasureString(label, -1, &font, layoutRect, &bounds);
    Gdiplus::REAL width = bounds.Width + static_cast<Gdiplus::REAL>(BUTTON_HORIZONTAL_PADDING * 2);
    return max(static_cast<Gdiplus::REAL>(46.0f), width);
}

int BuildActionButtons(
    Gdiplus::Graphics& graphics,
    const Gdiplus::Font& font,
    StatusActionSet actions,
    int windowWidth,
    ActionButton buttons[2]
) {
    int count = ActionCount(actions);
    if (count <= 0) return 0;

    buttons[0].action = STATUS_ACTION_RETRY;
    buttons[0].label = L"重试";
    buttons[0].rect.Width = ButtonWidth(graphics, font, buttons[0].label);
    buttons[0].rect.Height = static_cast<Gdiplus::REAL>(BUTTON_HEIGHT);
    if (count == 2) {
        buttons[1].action = STATUS_ACTION_CANCEL;
        buttons[1].label = L"取消";
        buttons[1].rect.Width = ButtonWidth(graphics, font, buttons[1].label);
        buttons[1].rect.Height = static_cast<Gdiplus::REAL>(BUTTON_HEIGHT);
    }

    Gdiplus::REAL totalWidth = 0.0f;
    for (int i = 0; i < count; ++i) {
        totalWidth += buttons[i].rect.Width;
    }
    totalWidth += static_cast<Gdiplus::REAL>(BUTTON_GAP * (count - 1));

    Gdiplus::REAL cursorX = static_cast<Gdiplus::REAL>(windowWidth - HORIZONTAL_PADDING) - totalWidth;
    for (int i = 0; i < count; ++i) {
        buttons[i].rect.X = cursorX;
        buttons[i].rect.Y = static_cast<Gdiplus::REAL>((WINDOW_HEIGHT - BUTTON_HEIGHT) / 2.0f);
        cursorX += buttons[i].rect.Width + static_cast<Gdiplus::REAL>(BUTTON_GAP);
    }
    return count;
}

// Paint the window
void PaintWindow(HDC hdc, int width, int height) {
    Gdiplus::Graphics graphics(hdc);
    graphics.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);
    graphics.SetTextRenderingHint(Gdiplus::TextRenderingHintClearTypeGridFit);

    // Clear background (transparent)
    graphics.Clear(Gdiplus::Color(0, 0, 0, 0));

    // Draw rounded rectangle background
    Gdiplus::GraphicsPath path;
    int r = CORNER_RADIUS;
    path.AddArc(0, 0, r * 2, r * 2, 180, 90);
    path.AddArc(width - r * 2, 0, r * 2, r * 2, 270, 90);
    path.AddArc(width - r * 2, height - r * 2, r * 2, r * 2, 0, 90);
    path.AddArc(0, height - r * 2, r * 2, r * 2, 90, 90);
    path.CloseFigure();

    StatusType status = STATUS_RECORDING;
    StatusActionSet actions = STATUS_ACTIONS_NONE;
    std::wstring text;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        status = g_currentStatus;
        actions = g_currentActions;
        text = g_currentText.empty() ? L" " : g_currentText;
    }

    Gdiplus::SolidBrush bgBrush(BACKGROUND_COLOR);
    graphics.FillPath(&bgBrush, &path);

    Gdiplus::Pen borderPen(BORDER_COLOR, 1.0f);
    graphics.DrawPath(&borderPen, &path);

    Gdiplus::SolidBrush dotBrush(STATUS_COLORS[status]);
    Gdiplus::RectF dotRect(
        static_cast<Gdiplus::REAL>(HORIZONTAL_PADDING),
        static_cast<Gdiplus::REAL>((height - DOT_SIZE) / 2.0f),
        static_cast<Gdiplus::REAL>(DOT_SIZE),
        static_cast<Gdiplus::REAL>(DOT_SIZE)
    );
    graphics.FillEllipse(&dotBrush, dotRect);

    // Draw text
    Gdiplus::FontFamily fontFamily(L"Segoe UI");
    Gdiplus::Font font(&fontFamily, 13, Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
    Gdiplus::Font buttonFont(&fontFamily, 12, Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
    Gdiplus::SolidBrush textBrush(TEXT_COLOR);

    Gdiplus::StringFormat format;
    format.SetAlignment(Gdiplus::StringAlignmentNear);
    format.SetLineAlignment(Gdiplus::StringAlignmentCenter);
    format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
    format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);

    ActionButton buttons[2];
    int buttonCount = BuildActionButtons(graphics, buttonFont, actions, width, buttons);
    Gdiplus::REAL textWidth = static_cast<Gdiplus::REAL>(max(0, width - HORIZONTAL_PADDING * 2 - DOT_SIZE - CONTENT_GAP));
    if (buttonCount > 0) {
        textWidth = max(
            static_cast<Gdiplus::REAL>(0.0f),
            buttons[0].rect.X - static_cast<Gdiplus::REAL>(ACTION_GAP) -
                static_cast<Gdiplus::REAL>(HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP)
        );
    }

    Gdiplus::RectF layoutRect(
        static_cast<Gdiplus::REAL>(HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP),
        0.0f,
        textWidth,
        static_cast<Gdiplus::REAL>(height)
    );

    graphics.DrawString(text.c_str(), -1, &font, layoutRect, &format, &textBrush);

    Gdiplus::SolidBrush buttonBrush(Gdiplus::Color(40, 255, 255, 255));
    Gdiplus::Pen buttonPen(Gdiplus::Color(56, 255, 255, 255), 1.0f);
    for (int i = 0; i < buttonCount; ++i) {
        ActionButton button = buttons[i];
        Gdiplus::GraphicsPath buttonPath;
        Gdiplus::REAL radius = static_cast<Gdiplus::REAL>(BUTTON_HEIGHT / 2);
        buttonPath.AddArc(button.rect.X, button.rect.Y, radius * 2, radius * 2, 180, 90);
        buttonPath.AddArc(button.rect.GetRight() - radius * 2, button.rect.Y, radius * 2, radius * 2, 270, 90);
        buttonPath.AddArc(button.rect.GetRight() - radius * 2, button.rect.GetBottom() - radius * 2, radius * 2, radius * 2, 0, 90);
        buttonPath.AddArc(button.rect.X, button.rect.GetBottom() - radius * 2, radius * 2, radius * 2, 90, 90);
        buttonPath.CloseFigure();
        graphics.FillPath(&buttonBrush, &buttonPath);
        graphics.DrawPath(&buttonPen, &buttonPath);

        Gdiplus::StringFormat buttonFormat;
        buttonFormat.SetAlignment(Gdiplus::StringAlignmentCenter);
        buttonFormat.SetLineAlignment(Gdiplus::StringAlignmentCenter);
        graphics.DrawString(button.label, -1, &buttonFont, button.rect, &buttonFormat, &textBrush);
    }
}

// Window procedure
LRESULT CALLBACK WindowProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam) {
    switch (msg) {
        case WM_PAINT: {
            PAINTSTRUCT ps;
            HDC hdc = BeginPaint(hwnd, &ps);
            
            // Create memory DC for double buffering
            HDC memDC = CreateCompatibleDC(hdc);
            int windowWidth = CalculateWindowWidth(hdc);
            HBITMAP memBitmap = CreateCompatibleBitmap(hdc, windowWidth, WINDOW_HEIGHT);
            HBITMAP oldBitmap = (HBITMAP)SelectObject(memDC, memBitmap);
            
            PaintWindow(memDC, windowWidth, WINDOW_HEIGHT);
            
            // Use UpdateLayeredWindow for proper transparency
            BLENDFUNCTION blend = {AC_SRC_OVER, 0, WINDOW_ALPHA, AC_SRC_ALPHA};
            POINT ptSrc = {0, 0};
            SIZE sizeWnd = {windowWidth, WINDOW_HEIGHT};
            POINT ptDst = CalculateWindowPosition(windowWidth);
            UpdateLayeredWindow(hwnd, hdc, &ptDst, &sizeWnd, memDC, &ptSrc, 0, &blend, ULW_ALPHA);
            
            SelectObject(memDC, oldBitmap);
            DeleteObject(memBitmap);
            DeleteDC(memDC);
            
            EndPaint(hwnd, &ps);
            return 0;
        }
        case WM_LBUTTONUP: {
            StatusActionSet actions = STATUS_ACTIONS_NONE;
            StatusOverlayActionCallback callback = nullptr;
            {
                std::lock_guard<std::mutex> lock(g_mutex);
                actions = g_currentActions;
                callback = g_actionCallback;
            }
            if (actions == STATUS_ACTIONS_NONE || !callback) {
                return 0;
            }

            int x = static_cast<short>(LOWORD(lParam));
            int y = static_cast<short>(HIWORD(lParam));
            HDC screenDC = GetDC(nullptr);
            Gdiplus::Graphics graphics(screenDC);
            Gdiplus::FontFamily fontFamily(L"Segoe UI");
            Gdiplus::Font buttonFont(&fontFamily, 12, Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
            int windowWidth = CalculateWindowWidth(screenDC);
            ActionButton buttons[2];
            int count = BuildActionButtons(graphics, buttonFont, actions, windowWidth, buttons);
            ReleaseDC(nullptr, screenDC);

            for (int i = 0; i < count; ++i) {
                const auto& rect = buttons[i].rect;
                if (x >= rect.X && x <= rect.GetRight() && y >= rect.Y && y <= rect.GetBottom()) {
                    callback(buttons[i].action);
                    return 0;
                }
            }
            return 0;
        }
        case WM_DESTROY:
            PostQuitMessage(0);
            return 0;
        default:
            return DefWindowProc(hwnd, msg, wParam, lParam);
    }
}

// Update and repaint window
void UpdateWindow() {
    if (!g_hwnd) return;
    
    // Create DC for layered window update
    HDC screenDC = GetDC(nullptr);
    HDC memDC = CreateCompatibleDC(screenDC);
    int windowWidth = CalculateWindowWidth(screenDC);
    
    BITMAPINFO bmi = {};
    bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bmi.bmiHeader.biWidth = windowWidth;
    bmi.bmiHeader.biHeight = -WINDOW_HEIGHT; // Top-down
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    
    void* bits = nullptr;
    HBITMAP memBitmap = CreateDIBSection(memDC, &bmi, DIB_RGB_COLORS, &bits, nullptr, 0);
    HBITMAP oldBitmap = (HBITMAP)SelectObject(memDC, memBitmap);
    
    // Clear to transparent
    memset(bits, 0, windowWidth * WINDOW_HEIGHT * 4);
    
    PaintWindow(memDC, windowWidth, WINDOW_HEIGHT);
    
    BLENDFUNCTION blend = {AC_SRC_OVER, 0, WINDOW_ALPHA, AC_SRC_ALPHA};
    POINT ptSrc = {0, 0};
    SIZE sizeWnd = {windowWidth, WINDOW_HEIGHT};
    POINT ptDst = CalculateWindowPosition(windowWidth);
    UpdateLayeredWindow(g_hwnd, screenDC, &ptDst, &sizeWnd, memDC, &ptSrc, 0, &blend, ULW_ALPHA);
    StatusActionSet actions = STATUS_ACTIONS_NONE;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        actions = g_currentActions;
    }
    LONG_PTR exStyle = GetWindowLongPtrW(g_hwnd, GWL_EXSTYLE);
    if (actions == STATUS_ACTIONS_NONE) {
        exStyle |= WS_EX_TRANSPARENT;
    } else {
        exStyle &= ~WS_EX_TRANSPARENT;
    }
    SetWindowLongPtrW(g_hwnd, GWL_EXSTYLE, exStyle);
    SetWindowPos(
        g_hwnd,
        HWND_TOPMOST,
        ptDst.x,
        ptDst.y,
        windowWidth,
        WINDOW_HEIGHT,
        SWP_NOACTIVATE | SWP_SHOWWINDOW
    );
    
    SelectObject(memDC, oldBitmap);
    DeleteObject(memBitmap);
    DeleteDC(memDC);
    ReleaseDC(nullptr, screenDC);
}

// Message loop thread
void MessageThreadProc() {
    // Initialize GDI+
    Gdiplus::GdiplusStartupInput gdiplusStartupInput;
    Gdiplus::GdiplusStartup(&g_gdiplusToken, &gdiplusStartupInput, nullptr);
    
    // Register window class
    WNDCLASSEXW wc = {};
    wc.cbSize = sizeof(WNDCLASSEXW);
    wc.lpfnWndProc = WindowProc;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.lpszClassName = WINDOW_CLASS_NAME;
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    RegisterClassExW(&wc);
    
    // Create layered window
    POINT pos = CalculateWindowPosition(MAX_WINDOW_WIDTH);
    g_hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE,
        WINDOW_CLASS_NAME,
        L"Status",
        WS_POPUP,
        pos.x, pos.y,
        MAX_WINDOW_WIDTH, WINDOW_HEIGHT,
        nullptr, nullptr,
        GetModuleHandle(nullptr),
        nullptr
    );
    
    if (!g_hwnd) {
        g_initialized = true;
        g_cv.notify_all();
        return;
    }
    
    // Signal initialization complete
    g_initialized = true;
    g_cv.notify_all();
    
    // Message loop
    MSG msg;
    while (!g_shouldExit) {
        while (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE)) {
            if (msg.message == WM_QUIT) {
                g_shouldExit = true;
                break;
            }
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
        
        // Check for custom messages via sleep (simple approach)
        Sleep(10);
    }
    
    // Cleanup
    if (g_hwnd) {
        DestroyWindow(g_hwnd);
        g_hwnd = nullptr;
    }
    UnregisterClassW(WINDOW_CLASS_NAME, GetModuleHandle(nullptr));
    Gdiplus::GdiplusShutdown(g_gdiplusToken);
}

} // anonymous namespace

extern "C" {

int status_overlay_init(void) {
    if (g_initialized) return 0;
    
    g_shouldExit = false;
    g_messageThread = std::thread(MessageThreadProc);
    
    // Wait for initialization
    std::unique_lock<std::mutex> lock(g_mutex);
    g_cv.wait(lock, [] { return g_initialized.load(); });
    
    return g_hwnd ? 0 : -1;
}

void status_overlay_show_actions(StatusType status, const char* text, StatusActionSet actions) {
    if (!g_hwnd) return;
    
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_currentStatus = status;
        g_currentActions = actions;
        g_currentText = Utf8ToWide(text);
    }
    
    UpdateWindow();
    ShowWindow(g_hwnd, SW_SHOWNOACTIVATE);
    g_visible = true;
}

void status_overlay_show(StatusType status, const char* text) {
    status_overlay_show_actions(status, text, STATUS_ACTIONS_NONE);
}

void status_overlay_set_action_callback(StatusOverlayActionCallback callback) {
    std::lock_guard<std::mutex> lock(g_mutex);
    g_actionCallback = callback;
}

void status_overlay_hide(void) {
    if (!g_hwnd) return;
    
    if (g_visible) {
        ShowWindow(g_hwnd, SW_HIDE);
        g_visible = false;
    }
}

void status_overlay_cleanup(void) {
    if (!g_initialized) return;
    
    g_shouldExit = true;
    
    if (g_hwnd) {
        PostMessage(g_hwnd, WM_QUIT, 0, 0);
    }
    
    if (g_messageThread.joinable()) {
        g_messageThread.join();
    }
    
    g_initialized = false;
    g_hwnd = nullptr;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_currentActions = STATUS_ACTIONS_NONE;
        g_actionCallback = nullptr;
    }
}

} // extern "C"
