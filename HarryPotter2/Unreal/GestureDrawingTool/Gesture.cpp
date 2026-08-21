//***********************************************************************
//
//	Gesture.cpp - the main source for the Gesture Drawing Tool.  If this
//		were a real app, it wouldn't all be in one file...
//	Author: Paul J. Furio
//
//***********************************************************************

#define OEMRESOURCE

#include <afxwin.h>
#include <afxdlgs.h>
#include "Resource.h"
#include "Gesture.h"

CMyApp myApp;

#define MIN_GESTURE_VALUE		0
#define MAX_GESTURE_VALUE		300
#define GESTURE_WINDOW_OFFSET	100

/////////////////////////////////////////////////////////////////////////
// CMyApp member functions

BOOL CMyApp::InitInstance ()
{
    m_pMainWnd = new CMainWindow;
    m_pMainWnd->ShowWindow (m_nCmdShow);
    m_pMainWnd->UpdateWindow ();
    return TRUE;
}

/////////////////////////////////////////////////////////////////////////
// CMainWindow message map and member functions

BEGIN_MESSAGE_MAP (CMainWindow, CFrameWnd)
    ON_WM_PAINT ()
    ON_COMMAND (IDM_FILE_NEW, OnFileNew)
    ON_COMMAND (IDM_FILE_EXIT, OnFileExit)
	ON_COMMAND (IDM_GESTURE_REFLECT, OnReflect)
 	ON_COMMAND (IDM_GESTURE_FLIP, OnFlip)
	ON_COMMAND (IDM_GESTURE_CENTER, OnCenter)
	ON_COMMAND (IDM_GESTURE_END, OnEndGesture)
	ON_COMMAND (IDM_FILE_SAVEAS, OnFileSaveAs)
	ON_COMMAND (IDM_FILE_SAVE, OnFileSave)
	ON_COMMAND (IDM_FILE_OPEN, OnFileOpen)
	ON_UPDATE_COMMAND_UI (IDM_FILE_NEW, OnUpdateFileNewUI)
	ON_UPDATE_COMMAND_UI (IDM_FILE_SAVE, OnUpdateFileNewUI)
	ON_UPDATE_COMMAND_UI (IDM_FILE_SAVEAS, OnUpdateFileNewUI)
	ON_UPDATE_COMMAND_UI (IDM_FILE_OPEN, OnUpdateFileOpenUI)
	ON_UPDATE_COMMAND_UI (IDM_GESTURE_END, OnUpdateFileOpenUI)
	ON_UPDATE_COMMAND_UI (IDM_GESTURE_REFLECT, OnUpdateReflectUI)
    ON_WM_LBUTTONDOWN ()
    ON_WM_MOUSEMOVE ()
    ON_WM_LBUTTONUP ()
    ON_WM_CONTEXTMENU ()
    ON_WM_MEASUREITEM ()
    ON_WM_DRAWITEM ()
END_MESSAGE_MAP ()

// Some constant values
const COLORREF gBlackColor = RGB (  0,   0,   0);
const COLORREF gRedColor = RGB (  255,   100,  100);
const COLORREF gYellowColor = RGB ( 255, 255, 100);
const char szFilter[] = "HP Gestures (*.hpg)|*.hpg|All Files (*.*)|*.*||";

CMainWindow::CMainWindow ()
{
	// Set the maximum size of the line segment array
    m_lineArray.SetSize (0, MAX_POINTS);

	// Set the window to be 300 by 300
	CRect		WinRect(GESTURE_WINDOW_OFFSET,GESTURE_WINDOW_OFFSET, 
		GESTURE_WINDOW_OFFSET + MAX_GESTURE_VALUE + 10, GESTURE_WINDOW_OFFSET + MAX_GESTURE_VALUE + 48);
		// There are crappy fudge factors above to make the drawing window more or less 300 x 300
	CPoint		HalfwayTopPoint(MAX_GESTURE_VALUE / 2,MIN_GESTURE_VALUE);
	CPoint		HalfwayBottomPoint(MAX_GESTURE_VALUE / 2,MAX_GESTURE_VALUE);

	CPoint		ATopPoint(MAX_GESTURE_VALUE / 3,MIN_GESTURE_VALUE);
	CPoint		ABottomPoint(MAX_GESTURE_VALUE / 3,MAX_GESTURE_VALUE);
	CPoint		BTopPoint(2 * MAX_GESTURE_VALUE / 3,MIN_GESTURE_VALUE);
	CPoint		BBottomPoint(2 * MAX_GESTURE_VALUE / 3,MAX_GESTURE_VALUE);
	CPoint		CLeftPoint(MIN_GESTURE_VALUE,MAX_GESTURE_VALUE/3);
	CPoint		CRightPoint(MAX_GESTURE_VALUE,MAX_GESTURE_VALUE/3);
	CPoint		DLeftPoint(MIN_GESTURE_VALUE,2*MAX_GESTURE_VALUE/3);
	CPoint		DRightPoint(MAX_GESTURE_VALUE,2*MAX_GESTURE_VALUE/3);


	// Generate Static Lines
	m_MidwayLine = new CLine(HalfwayTopPoint,HalfwayBottomPoint);
	m_SegLineA = new CLine(ATopPoint,ABottomPoint);
	m_SegLineB = new CLine(BTopPoint,BBottomPoint);
	m_SegLineC = new CLine(CLeftPoint,CRightPoint);
	m_SegLineD = new CLine(DLeftPoint,DRightPoint);

    CString strWndClass = AfxRegisterWndClass (
        0,
        myApp.LoadStandardCursor (IDC_CROSS),
        (HBRUSH) (COLOR_WINDOW + 1),
        myApp.LoadIcon (IDR_MAINFRAME)
    );
    
    Create (strWndClass, "Gesture Drawing Tool", WS_OVERLAPPED | WS_MINIMIZEBOX | WS_CAPTION | WS_SYSMENU, 
        WinRect, NULL, MAKEINTRESOURCE (IDR_MAINFRAME));

    LoadAccelTable (MAKEINTRESOURCE (IDR_MAINFRAME));

	CMenu* pMenu = GetMenu();
	pMenu->EnableMenuItem(IDM_FILE_OPEN,1);


	m_szFilename[0] = '\0';
	m_bReflect = false;
}

CMainWindow::~CMainWindow ()
{
    DeleteAllLines ();
}

void CMainWindow::OnPaint ()
{
    CPaintDC dc (this);
	
	// Draw the Halfway Line First
	m_MidwayLine->Draw(&dc, gRedColor);

	m_SegLineA->Draw(&dc, gYellowColor);
	m_SegLineB->Draw(&dc, gYellowColor);
	m_SegLineC->Draw(&dc, gYellowColor);
	m_SegLineD->Draw(&dc, gYellowColor);


    int nCount = m_lineArray.GetSize ();

    if (nCount) {
        for (int i=0; i<nCount; i++)
            ((CLine*) m_lineArray[i])->Draw (&dc, gBlackColor);
    }
}

// Clear the Gesture
void CMainWindow::OnFileNew ()
{
    DeleteAllLines ();
	m_szFilename[0]='\0';
    Invalidate ();
}

// Un-Gray out some menu items
void CMainWindow::OnUpdateFileNewUI (CCmdUI* pCmdUI)
{
    pCmdUI->Enable (m_lineArray.GetSize ());
}

void CMainWindow::OnUpdateFileOpenUI (CCmdUI* pCmdUI)
{
	pCmdUI->Enable(TRUE);
}

// Add a check when Reflect is Enabled
void CMainWindow::OnUpdateReflectUI(CCmdUI* pCmdUI)
{
	pCmdUI->SetCheck(m_bReflect);
}

void CMainWindow::OnFileExit ()
{
    SendMessage (WM_CLOSE, 0, 0);
}


// Save an existing shape that's been altered
void CMainWindow::OnFileSave ()
{
	if(m_szFilename[0] == '\0')
	{
		OnFileSaveAs();
		return;
	}

	ConvertGestureFromLineArray();

	SaveFile();
}

// Save to a new filename
void CMainWindow::OnFileSaveAs ()
{
	CFileDialog fDlg(FALSE,"hpg",NULL, OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,szFilter,this);

	if(fDlg.DoModal() == IDOK)
	{
		strcpy(m_szFilename,fDlg.GetPathName());
		if(m_szFilename[0] != '\0')
		{
			ConvertGestureFromLineArray();
			
			SaveFile();
		}		
	}

}

// The actual save code, that uses boring old C file manipulation routines
void CMainWindow::SaveFile()
{
	FILE *lpFile = fopen(m_szFilename,"wb");

	if(lpFile)
	{
		short nNumPoints = m_Gesture.GetNumPoints();
		short i;

		// HPGF is the header for a valid file.
		fwrite("HPGF",sizeof(char),4,lpFile);
		fwrite(&nNumPoints,sizeof(short),1,lpFile);
		
		for(i = 0; i < nNumPoints; i++)
		{
			fwrite(&m_Gesture.GetPoint(i)->m_fXVal,sizeof(float),1,lpFile);
			fwrite(&m_Gesture.GetPoint(i)->m_fYVal,sizeof(float),1,lpFile);
		}

		for(i=0; i < 32; i++)
		{
			fwrite(&m_Gesture.m_nSectorsArray[i],sizeof(short),1,lpFile);
		}

		fclose(lpFile);
	}
}


void CMainWindow::OnFileOpen ()
{
	CFileDialog fDlg(TRUE,"hpg",NULL,NULL,szFilter,this);

	if(fDlg.DoModal() == IDOK)
	{
	   DeleteAllLines ();
		m_szFilename[0]='\0';
	    Invalidate ();

		strcpy(m_szFilename,fDlg.GetPathName());
		
		if(m_szFilename[0] != '\0')
		{
			FILE *lpFile = fopen(m_szFilename,"rb");
			if(lpFile)
			{
				char szBuffer[128];

				if(fread(szBuffer,sizeof(char),4,lpFile))
				{
					szBuffer[4] = '\0';
					if(strcmp(szBuffer,"HPGF")!=0)
					{
						// This should really put of a dialog that reads "Invalid File Format".
						//	Maybe it will in version 2
						fclose(lpFile);
						return;
					}
					
					m_Gesture.ClearGesture();
					short nNumPoints;

					if(fread(&nNumPoints,sizeof(short),1,lpFile) != 1)
					{
						fclose(lpFile);
						return;
					}

					CFloatPoint *lpTempPoint;
					float fX, fY;
					int nXResult, nYResult;

					for(short i=0; i < nNumPoints; i++)
					{


						nXResult = fread(&fX,sizeof(float),1,lpFile);
						nYResult = fread(&fY,sizeof(float),1,lpFile);

						if((nXResult !=1)||(nYResult!=1))
						{
							if(feof(lpFile))
							{
								m_Gesture.ClearGesture();
								fclose(lpFile);
								return;
							}
						}

						lpTempPoint = new CFloatPoint(fX,fY);

						m_Gesture.AddPoint(lpTempPoint);
					}
					
					ConvertGestureToLineArray();

					Invalidate();
				}

				fclose(lpFile);
			}
		}
	}
}

// Flip the Gesture left to right
void CMainWindow::OnFlip ()
{
	int nCount = m_lineArray.GetSize ();
	long nMid = MAX_GESTURE_VALUE/2;

	if(nCount)
	{
		for (int i=0; i < nCount; i++)
		{
			((CLine*) m_lineArray[i])->ReflectAboutMiddle();
		}
	}

	Invalidate();
}

// Center the Gesture in the window
void CMainWindow::OnCenter ()
{
	// Find Max and Min and adjust points...
	int nCount = m_lineArray.GetSize ();
	int i;

	if (nCount) 
	{
		long nMaxX, nMinX;
		long nAdjustment = 0;
		long nX;

		nMinX = MAX_GESTURE_VALUE;
		nMaxX = MIN_GESTURE_VALUE;

		// Get the Ranges First
		for (i=0; i<nCount; i++)
		{
            nX = ((CLine*) m_lineArray[i])->GetFromPoint().x;

			if(nMinX > nX) nMinX = nX;
			if(nMaxX < nX) nMaxX = nX;
		}

		nAdjustment = (MAX_GESTURE_VALUE / 2) - (nMinX + ((nMaxX - nMinX) / 2));

	// Now Perform the Centering Adjustment
        for (i=0; i<nCount; i++)
		{
			((CLine*) m_lineArray[i])->AdjustX(nAdjustment);
		}
	}

	// Force a repaint
	Invalidate ();
}

// Auto reflect the Gesture around the center (red) line
void CMainWindow::OnReflect ()
{
	m_bReflect = !m_bReflect;
	CLine *pLine;
	CPoint ptFrom, ptTo;

	int nCount = m_lineArray.GetSize ();

	if(m_bReflect)
	{
		// Repeat the lines inversed
	    if (nCount)
		{
			CClientDC dc (this);
			long nMid = MAX_GESTURE_VALUE/2;

			for (int i=nCount-1; i>=0; i--)
			{
				ptFrom = ((CLine*) m_lineArray[i])->GetFromPoint();
				ptTo = ((CLine*) m_lineArray[i])->GetToPoint();

				ptFrom.x = nMid + (nMid - ptFrom.x);
				ptTo.x = nMid + (nMid - ptTo.x);

				pLine = new CLine(ptFrom,ptTo);

				pLine->Draw(&dc,gBlackColor);
				m_lineArray.Add (pLine);
			}
	    }
	}
	else	// Delete half the lines
	{
	    if (nCount)
		{		
			int nHalfwayPoint = nCount / 2;

			m_lineArray.RemoveAt(nHalfwayPoint,nHalfwayPoint);
	    }
	}

	Invalidate();
}

// End the gesture, stop drawing lines
void CMainWindow::OnEndGesture()
{
    if (GetCapture () == this) {
        ReleaseCapture ();

        CClientDC dc (this);
        InvertLine (&dc, m_ptFrom, m_ptTo);
        CLine* pLine = NULL;

        try {
            pLine = new CLine (m_ptFrom, m_ptTo);

            m_lineArray.Add (pLine);
            pLine->Draw (&dc, gBlackColor);
        }
        catch (CMemoryException* e) {
            MessageBox ("Out of memory. You must clear the " \
                "drawing area before adding more lines.", "Error",
                MB_ICONEXCLAMATION | MB_OK);

            if (pLine != NULL)
                delete pLine;
            e->Delete ();   
        }       
    }
}

// The mouse follow and control routines, stolen from the Prosise Win95 MFC book
void CMainWindow::OnLButtonDown (UINT nFlags, CPoint point)
{
	if(GetCapture () != this)
	{
	    m_ptFrom = point;
	    m_ptTo = point;
	    SetCapture ();
	}
}

void CMainWindow::OnMouseMove (UINT nFlags, CPoint point)
{
    if (GetCapture () == this) {
        CClientDC dc (this);
        InvertLine (&dc, m_ptFrom, m_ptTo);
        InvertLine (&dc, m_ptFrom, point);
        m_ptTo = point;
    }
}

void CMainWindow::OnLButtonUp (UINT nFlags, CPoint point)
{
    if (GetCapture () == this) {
        ReleaseCapture ();

        CClientDC dc (this);
        InvertLine (&dc, m_ptFrom, m_ptTo);
        CLine* pLine = NULL;

        try {
            pLine = new CLine (m_ptFrom, m_ptTo);

            m_lineArray.Add (pLine);
            pLine->Draw (&dc, gBlackColor);
        }
        catch (CMemoryException* e) {
            MessageBox ("Out of memory. You must clear the " \
                "drawing area before adding more lines.", "Error",
                MB_ICONEXCLAMATION | MB_OK);

            if (pLine != NULL)
                delete pLine;
            e->Delete ();   
        }
		
		m_ptFrom = point;
	    m_ptTo = point;
	    SetCapture ();
    }
}

// The context menu, also stolen from Prosise
void CMainWindow::OnContextMenu (CWnd* pWnd, CPoint point)
{
    CRect rect;
    GetClientRect (&rect);
    ClientToScreen (&rect);

    if (rect.PtInRect (point)) {
        CMenu menu;
        menu.LoadMenu (IDR_CONTEXTMENU);
        CMenu* pContextMenu = menu.GetSubMenu (0);

        pContextMenu->TrackPopupMenu (TPM_LEFTALIGN | TPM_LEFTBUTTON |
            TPM_RIGHTBUTTON, point.x, point.y, this);
        return;
    }
    CFrameWnd::OnContextMenu (pWnd, point);
}

void CMainWindow::OnMeasureItem (int nIDCtl, LPMEASUREITEMSTRUCT lpmis)
{
    lpmis->itemWidth = ::GetSystemMetrics (SM_CYMENU) * 4;
    lpmis->itemHeight = ::GetSystemMetrics (SM_CYMENU);
}

void CMainWindow::OnDrawItem (int nIDCtl, LPDRAWITEMSTRUCT lpdis)
{
    BITMAP bm;
    CBitmap bitmap;
    bitmap.LoadOEMBitmap (OBM_CHECK);
    bitmap.GetObject (sizeof (bm), &bm);

    CDC dc;
    dc.Attach (lpdis->hDC);

    CBrush* pBrush = new CBrush (::GetSysColor ((lpdis->itemState &
        ODS_SELECTED) ? COLOR_HIGHLIGHT : COLOR_MENU));
    dc.FrameRect (&(lpdis->rcItem), pBrush);
    delete pBrush;

    if (lpdis->itemState & ODS_CHECKED) {
        CDC dcMem;
        dcMem.CreateCompatibleDC (&dc);
        CBitmap* pOldBitmap = dcMem.SelectObject (&bitmap);

        dc.BitBlt (lpdis->rcItem.left + 4, lpdis->rcItem.top +
            (((lpdis->rcItem.bottom - lpdis->rcItem.top) -
            bm.bmHeight) / 2), bm.bmWidth, bm.bmHeight, &dcMem,
            0, 0, SRCCOPY);

        dcMem.SelectObject (pOldBitmap);
    }

    pBrush = new CBrush (gBlackColor);
    CRect rect = lpdis->rcItem;
    rect.DeflateRect (6, 4);
    rect.left += bm.bmWidth;
    dc.FillRect (rect, pBrush);
    delete pBrush;

    dc.Detach ();
}

void CMainWindow::InvertLine (CDC* pDC, CPoint ptFrom, CPoint ptTo)
{
    int nOldMode = pDC->SetROP2 (R2_NOT);

    pDC->MoveTo (ptFrom);
    pDC->LineTo (ptTo);

    pDC->SetROP2 (nOldMode);
}

void CMainWindow::DeleteAllLines ()
{
    int nCount = m_lineArray.GetSize ();

    for (int i=0; i<nCount; i++)
        delete m_lineArray[i];

    m_lineArray.RemoveAll ();
}

// Some conversion routines to the Gesture Class.  This is a hokey intermediate class
//	for storing the drawing as relative floats instead of absolute screen coordinates
void CMainWindow::ConvertGestureFromLineArray()
{
	int nCount = m_lineArray.GetSize ();
	long nX, nY;
	float fAdjustedX, fAdjustedY;
	long nMinX, nMaxX, nMinY, nMaxY;
	long nXMidPoint, nYRange;
	CFloatPoint * TempPoint;
	int i;

	m_Gesture.ClearGesture();

	// Find Absolutes
	nMinY = MAX_GESTURE_VALUE;	
	nMaxY = MIN_GESTURE_VALUE;
	nMinX = MAX_GESTURE_VALUE;
	nMaxX = MIN_GESTURE_VALUE;

	if (nCount) 
	{
		for (i=0; i<nCount; i++)
		{
            nX = ((CLine *)m_lineArray[i])->GetFromPoint().x;
			nY = ((CLine *)m_lineArray[i])->GetFromPoint().y;
 
			if(nMinY > nY) nMinY = nY;
			if(nMaxY < nY) nMaxY = nY;
			if(nMinX > nX) nMinX = nX;
			if(nMaxX < nX) nMaxX = nX;
		}

		nXMidPoint = ((nMaxX - nMinX) / 2) + nMinX;
		nYRange = nMaxY - nMinY;

	// Now Perform the Conversion

		// Get First Point
        nX = ((CLine *)m_lineArray[0])->GetFromPoint().x;
		nY = ((CLine *)m_lineArray[0])->GetFromPoint().y;

		// Convert to Absolutes
		fAdjustedX = ((float) ( nX ) / ((float) nYRange));
		fAdjustedY = ((float) ( nY - nMinY)) / ((float) nYRange);	// Assume the Gesture is higher than wider

		TempPoint = new CFloatPoint(fAdjustedX, fAdjustedY);
			
		m_Gesture.AddPoint(TempPoint);

        for (i=0; i<nCount; i++)
		{
            nX = ((CLine *)m_lineArray[i])->GetToPoint().x;
			nY = ((CLine *)m_lineArray[i])->GetToPoint().y;

			// Convert to Absolutes
			fAdjustedX = ((float) ( nX )) / ((float) nYRange);
			fAdjustedY = ((float) ( nY - nMinY)) / ((float) nYRange);

			TempPoint = new CFloatPoint(fAdjustedX, fAdjustedY);
			
			m_Gesture.AddPoint(TempPoint);
		}


		// Now go through the points and make the grid array;
		i=0;
		int j=0;
		short nLastSegment = 0;
		short nCurrentSegment = 0;
		int nNumFloatPoints = m_Gesture.GetNumPoints();

		while(i < nNumFloatPoints)
		{
			nCurrentSegment = PlaceSegment(m_Gesture.GetPoint(i));

			if(nCurrentSegment != nLastSegment)
			{
				m_Gesture.m_nSectorsArray[j] = nCurrentSegment;
				nLastSegment = nCurrentSegment;
				j++;
			}

			i++;
		}

		while(j<32)
		{
			m_Gesture.m_nSectorsArray[j] = 0;
			j++;
		}
    }
}

void CMainWindow::ConvertGestureToLineArray()
{
	int i;
	CPoint ptFrom, ptTo;
	CFloatPoint *lpGptFrom, *lpGptTo;
	CLine* pLine = NULL;

	if(m_Gesture.GetNumPoints() == 0)
		return;

	// Clear the buffer
	for(i=0;i<32;i++)
		m_Gesture.m_nSectorsArray[i] = 0;

	lpGptFrom = m_Gesture.GetPoint(0);

	ptFrom.x = (long) (lpGptFrom->m_fXVal * (float) MAX_GESTURE_VALUE);
	ptFrom.y = (long) (lpGptFrom->m_fYVal * (float) MAX_GESTURE_VALUE);

	for(i=1; i < m_Gesture.GetNumPoints(); i++)
	{
		lpGptTo = m_Gesture.GetPoint(i);

		ptTo.x = (long) (lpGptTo->m_fXVal * (float) MAX_GESTURE_VALUE);
		ptTo.y = (long) (lpGptTo->m_fYVal * (float) MAX_GESTURE_VALUE);	

        pLine = new CLine (ptFrom, ptTo);

        m_lineArray.Add (pLine);

		ptFrom = ptTo;
	}
}

short CMainWindow::PlaceSegment(CFloatPoint *lpPoint)
{
	if(lpPoint->m_fXVal <= 0.33)
	{
		if(lpPoint->m_fYVal <= 0.33)
			return 1;
		else if(lpPoint->m_fYVal <= 0.66)
			return 4;
		else
			return 7;
	}
	else if(lpPoint->m_fXVal <= 0.66)
	{
		if(lpPoint->m_fYVal <= 0.33)
			return 2;
		else if(lpPoint->m_fYVal <= 0.66)
			return 5;
		else
			return 8;
	}
	else
	{
		if(lpPoint->m_fYVal <= 0.33)
			return 3;
		else if(lpPoint->m_fYVal <= 0.66)
			return 6;
		else
			return 9;
	}
	return 0;
}


/////////////////////////////////////////////////////////////////////////
// CLine member functions

CLine::CLine (CPoint ptFrom, CPoint ptTo)
{
    m_ptFrom = ptFrom;
    m_ptTo = ptTo;
}

void CLine::Draw (CDC* pDC, COLORREF Color)
{
    CPen pen (PS_SOLID, 1, Color);

    CPen* pOldPen = pDC->SelectObject (&pen);
    pDC->MoveTo (m_ptFrom);
    pDC->LineTo (m_ptTo);

    pDC->SelectObject (pOldPen);
}

void CLine::ReflectAboutMiddle()
{
	long nMid = MAX_GESTURE_VALUE/2;

	m_ptFrom.x = nMid + (nMid - m_ptFrom.x);
	m_ptTo.x  = nMid + (nMid - m_ptTo.x);
}

void CLine::AdjustX(long nAdjustmentValue)
{
	m_ptFrom.x += nAdjustmentValue;
	m_ptTo.x  += nAdjustmentValue;
}

//////////////////////////////////////////////////////////////////////////
// CGesture member Functions

CGesture::CGesture()
{
	memset(m_Points,NULL,MAX_POINTS);
	m_nNumPoints = 0;
}

void CGesture::ClearGesture()
{
	for(int i = 0;i<m_nNumPoints;i++)
	{
		delete m_Points[i];
		m_Points[i] = NULL;
	}

	m_nNumPoints = 0;
}

void CGesture::AddPoint(CFloatPoint *lpPoint)
{
	if((m_nNumPoints + 1) < MAX_POINTS)
	{
		m_Points[m_nNumPoints] = lpPoint;
		m_nNumPoints++;
	}
}


///////////////////////////////////////////////////////////////////////
// CFloatPoint constructor

CFloatPoint::CFloatPoint(float X,float Y)
{
	m_fXVal = X;
	m_fYVal = Y;
}
