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
std::wstring g_currentText;

const wchar_t* WINDOW_CLASS_NAME = L"VTTStatusOverlay";

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
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        text = g_currentText.empty() ? L" " : g_currentText;
    }

    Gdiplus::Graphics graphics(hdc);
    Gdiplus::FontFamily fontFamily(L"Segoe UI");
    Gdiplus::Font font(&fontFamily, 13, Gdiplus::FontStyleBold, Gdiplus::UnitPixel);
    Gdiplus::RectF layoutRect(0.0f, 0.0f, static_cast<Gdiplus::REAL>(MAX_WINDOW_WIDTH), static_cast<Gdiplus::REAL>(WINDOW_HEIGHT));
    Gdiplus::RectF bounds;
    graphics.MeasureString(text.c_str(), -1, &font, layoutRect, &bounds);

    int textWidth = static_cast<int>(std::ceil(bounds.Width));
    int idealWidth = HORIZONTAL_PADDING * 2 + DOT_SIZE + CONTENT_GAP + textWidth;
    return min(MAX_WINDOW_WIDTH, max(1, idealWidth));
}

// Calculate window position (bottom center)
POINT CalculateWindowPosition(int windowWidth) {
    RECT workArea = GetWorkArea();
    int screenWidth = workArea.right - workArea.left;
    int x = workArea.left + (screenWidth - windowWidth) / 2;
    int y = workArea.bottom - WINDOW_HEIGHT - BOTTOM_MARGIN;
    return {x, y};
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
    std::wstring text;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        status = g_currentStatus;
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
    Gdiplus::SolidBrush textBrush(TEXT_COLOR);

    Gdiplus::StringFormat format;
    format.SetAlignment(Gdiplus::StringAlignmentNear);
    format.SetLineAlignment(Gdiplus::StringAlignmentCenter);
    format.SetTrimming(Gdiplus::StringTrimmingEllipsisCharacter);
    format.SetFormatFlags(Gdiplus::StringFormatFlagsNoWrap);

    Gdiplus::RectF layoutRect(
        static_cast<Gdiplus::REAL>(HORIZONTAL_PADDING + DOT_SIZE + CONTENT_GAP),
        0.0f,
        static_cast<Gdiplus::REAL>(max(0, width - HORIZONTAL_PADDING * 2 - DOT_SIZE - CONTENT_GAP)),
        static_cast<Gdiplus::REAL>(height)
    );

    graphics.DrawString(text.c_str(), -1, &font, layoutRect, &format, &textBrush);
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

void status_overlay_show(StatusType status, const char* text) {
    if (!g_hwnd) return;
    
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_currentStatus = status;
        g_currentText = Utf8ToWide(text);
    }
    
    UpdateWindow();
    ShowWindow(g_hwnd, SW_SHOWNOACTIVATE);
    g_visible = true;
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
}

} // extern "C"
