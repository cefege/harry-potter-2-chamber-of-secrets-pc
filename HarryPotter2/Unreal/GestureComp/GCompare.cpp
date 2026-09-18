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
#include "GCompare.h"

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
    ON_COMMAND (IDM_FILE_CLOSE, OnFileClose)
    ON_COMMAND (IDM_FILE_EXIT, OnFileExit)
	ON_COMMAND (IDM_FILE_OPEN, OnFileOpen)
	ON_COMMAND (IDM_FILE_CLEAR, OnFileClear)
	ON_COMMAND_RANGE (IDM_LEVEL1, IDM_LEVEL3, OnLevel)
	ON_UPDATE_COMMAND_UI (IDM_FILE_CLOSE, OnUpdateUI)
	ON_UPDATE_COMMAND_UI (IDM_FILE_CLEAR, OnUpdateUI)
	ON_UPDATE_COMMAND_UI_RANGE (IDM_LEVEL1, IDM_LEVEL3, OnUpdateLevel)
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
const char szFilter[] = "HP Gestures (*.hpg)|*.hpg|All Files (*.*)|*.*||";

short PlaceSegment(float fXVal, float fYVal);


CMainWindow::CMainWindow ()
{
	// Set the maximum size of the line segment array
    m_lineArray.SetSize (0, MAX_POINTS);
    m_DrawnPointsArray.SetSize (0, MAX_POINTS);

	// Set the window to be 300 by 300
	CRect		WinRect(GESTURE_WINDOW_OFFSET,GESTURE_WINDOW_OFFSET, 
		GESTURE_WINDOW_OFFSET + MAX_GESTURE_VALUE + 10, GESTURE_WINDOW_OFFSET + MAX_GESTURE_VALUE + 48);
		// There are crappy fudge factors above to make the drawing window more or less 300 x 300
	CPoint		HalfwayTopPoint(MAX_GESTURE_VALUE / 2,MIN_GESTURE_VALUE);
	CPoint		HalfwayBottomPoint(MAX_GESTURE_VALUE / 2,MAX_GESTURE_VALUE);
	m_MidwayLine = new CLine(HalfwayTopPoint,HalfwayBottomPoint);

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

	m_nLevel = IDM_LEVEL1;
	m_szFilename[0] = '\0';
	m_bReflect = false;
}

CMainWindow::~CMainWindow ()
{
    DeleteAllLines ();
	DeleteAllPoints ();
}

void CMainWindow::OnPaint ()
{
    CPaintDC dc (this);
	
    int nCount = m_lineArray.GetSize ();

    if (nCount) {
        for (int i=0; i<nCount; i++)
            ((CLine*) m_lineArray[i])->Draw (&dc, gRedColor);
    }
}

// Clear the Gesture
void CMainWindow::OnFileClose ()
{
    DeleteAllLines ();
	DeleteAllPoints ();
	m_szFilename[0]='\0';
    Invalidate ();
}

void CMainWindow::OnFileClear ()
{
	DeleteAllPoints ();
	Invalidate ();
}

// Un-Gray out some menu items
void CMainWindow::OnUpdateUI (CCmdUI* pCmdUI)
{
    pCmdUI->Enable (m_lineArray.GetSize ());
}

void CMainWindow::OnLevel (UINT nID)
{
    m_nLevel = nID;
}

void CMainWindow::OnUpdateLevel (CCmdUI* pCmdUI)
{
    pCmdUI->SetCheck (pCmdUI->m_nID == m_nLevel);
}


// Add a check when Reflect is Enabled
//	pCmdUI->SetCheck(m_bReflect);

void CMainWindow::OnFileExit ()
{
    SendMessage (WM_CLOSE, 0, 0);
}



void CMainWindow::OnFileOpen ()
{
	CFileDialog fDlg(TRUE,"hpg",NULL,NULL,szFilter,this);

	if(fDlg.DoModal() == IDOK)
	{
		DeleteAllLines ();
		DeleteAllPoints ();
		m_szFilename[0]='\0';
	    Invalidate ();

		strcpy(m_szFilename,fDlg.GetPathName());
		
		if(m_szFilename[0] != '\0')
		{
			FILE *lpFile = fopen(m_szFilename,"r");
			if(lpFile)
			{
				char szBuffer[128];
				short i;

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
					short nNumPoints = 0;

					if(fread(&nNumPoints,sizeof(short),1,lpFile) != 1)
					{
						fclose(lpFile);
						return;
					}

					CFloatPoint *lpTempPoint;
					float fX, fY;

					for(i=0; i < nNumPoints; i++)
					{
						if((fread(&fX,sizeof(float),1,lpFile)!=1) ||
						(fread(&fY,sizeof(float),1,lpFile)!=1))
						{
							m_Gesture.ClearGesture();
							fclose(lpFile);
							return;
						}

						lpTempPoint = new CFloatPoint(fX,fY);

						m_Gesture.AddPoint(lpTempPoint);
					}
					
					int nResult;
					// Read out the segments order
					for(i=0;i<SEG_SECTIONS;i++)
					{
						nResult = fread(&m_Gesture.m_nSegments[i],sizeof(short),1,lpFile);

						if(nResult != 1)
						{
							nResult = ferror(lpFile);
							nResult = feof(lpFile);
							m_Gesture.ClearGesture();
							fclose(lpFile);
							return;
						}
					}
					
					ConvertGestureToLineArray();

					Invalidate();
				}

				fclose(lpFile);
			}
		}
	}
}


// The mouse follow and control routines, stolen from the Prosise Win95 MFC book
void CMainWindow::OnLButtonDown (UINT nFlags, CPoint point)
{
	if(GetCapture () != this)
	{
        CDrawnPoint *lpPoint = NULL;

		try
		{
			lpPoint = new CDrawnPoint(point);
			// Add point to Array

		    m_DrawnPointsArray.Add (lpPoint);

			// Draw the point
			CClientDC dc (this);
			lpPoint->Draw(&dc,gBlackColor);

		   SetCapture ();
		}
		catch (CMemoryException* e) 
		{
			MessageBox ("Out of memory. You must clear the " \
				"drawing area before drawing your Gesture.", "Error",
				MB_ICONEXCLAMATION | MB_OK);

			if (lpPoint != NULL)
				delete lpPoint;
			e->Delete ();   
		}
	}
}

void CMainWindow::OnMouseMove (UINT nFlags, CPoint point)
{
    if (GetCapture () == this) 
	{
        CDrawnPoint *lpPoint = NULL;

		try
		{
			lpPoint = new CDrawnPoint(point);
			// Add point to Array

		    m_DrawnPointsArray.Add (lpPoint);

			// Draw the point
			CClientDC dc (this);
			lpPoint->Draw(&dc,gBlackColor);
		}
		catch (CMemoryException* e) 
		{
			MessageBox ("Out of memory. You must clear the " \
				"drawing area before drawing your Gesture.", "Error",
				MB_ICONEXCLAMATION | MB_OK);

			if (lpPoint != NULL)
				delete lpPoint;
			e->Delete ();   
		}
    }

}

void CMainWindow::OnLButtonUp (UINT nFlags, CPoint point)
{
    if (GetCapture () == this)
	{
        ReleaseCapture ();

		CompareGesture(); 	// Do Compare...
    }
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

void CMainWindow::DeleteAllPoints ()
{
    int nCount = m_DrawnPointsArray.GetSize ();

    for (int i=0; i<nCount; i++)
        delete m_DrawnPointsArray[i];

    m_DrawnPointsArray.RemoveAll ();

}


void CMainWindow::ConvertGestureToLineArray()
{
	int i;
	CPoint ptFrom, ptTo;
	CFloatPoint *lpGptFrom, *lpGptTo;
	CLine* pLine = NULL;

	if(m_Gesture.GetNumPoints() == 0)
		return;

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



void CMainWindow::CompareGesture ()
{
	int nCount = m_DrawnPointsArray.GetSize ();
	char szResults[256];
	short nDrawnSectors[SEG_SECTIONS * 3];
	int j;
	float fAdjustedX, fAdjustedY;
	int i;

	szResults[0] = '\0';

	// Brilliant comparison algorithm here...
	
	int nMinX = MAX_GESTURE_VALUE, nMinY = MAX_GESTURE_VALUE;
	int nMaxX = 0, nMaxY = 0;
	int nX, nY;
	int nXRange, nYRange;

	// First get the Min and Max of the drawn points.
	for (i=0; i<nCount; i++)
	{
		nX = ((CDrawnPoint *)m_DrawnPointsArray[i])->GetPoint().x;
		nY = ((CDrawnPoint *)m_DrawnPointsArray[i])->GetPoint().y;

		if(nMinY > nY) nMinY = nY;
		if(nMaxY < nY) nMaxY = nY;
		if(nMinX > nX) nMinX = nX;
		if(nMaxX < nX) nMaxX = nX;
	}

	nXRange = nMaxX - nMinX;
	nYRange = nMaxY - nMinY;

	// Now convert all the points to normalized floats and figure out what segments they're in,
	//	calculating the order on the fly

	for(j=0;j < SEG_SECTIONS;j++)
		nDrawnSectors[j] = 0;

	j=0;
	int nCurrentSegment = 0, nLastSegment = 0;

	for(i=0; i<nCount; i++)
	{
        nX = ((CDrawnPoint *)m_DrawnPointsArray[i])->GetPoint().x;
		nY = ((CDrawnPoint *)m_DrawnPointsArray[i])->GetPoint().y;

		// Convert to Absolutes
		fAdjustedX = ((float) ( nX - nMinX)) / ((float) nXRange);
		fAdjustedY = ((float) ( nY - nMinY)) / ((float) nYRange);

		nCurrentSegment = PlaceSegment(fAdjustedX,fAdjustedY);

		if(nCurrentSegment != nLastSegment)
		{
			nLastSegment = nCurrentSegment;
			nDrawnSectors[j] = nCurrentSegment;
			j++;
			if(j >= (SEG_SECTIONS * 3))
			{
				MessageBox("Point Overflow","Gesture Results");
				return;
			}
		}		
	}


	// Adjust for Level here by copying into a temp buffer...

	unsigned short nAdjustedGestureArray[SEG_SECTIONS * 3];
	i=0;
	j=0;

	while(m_Gesture.m_nSegments[i] != 0)
	{
		nAdjustedGestureArray[j] = m_Gesture.m_nSegments[i];
		i++;
		j++;
	}
	if(m_nLevel >= IDM_LEVEL2)
	{
		i = i -2;
		
		while(i >= 0)
		{
			nAdjustedGestureArray[j] = m_Gesture.m_nSegments[i];
			i--;
			j++;
		}

		if(m_nLevel >= IDM_LEVEL3)
		{
			i=1;
			while(m_Gesture.m_nSegments[i] != 0)
			{
				nAdjustedGestureArray[j] = m_Gesture.m_nSegments[i];
				i++;
				j++;
			}
		}
	}
	nAdjustedGestureArray[j] = 0; // "Terminate" the array with a '0'



	// Match Up Points with Gesture Order - Only works on Level 1 so far, because we're not checking m_nLevel
	i=0;
	j=0;
	int nNumCorrectSpots = 0;
	int nNumErrors = 0;
	float fPercentCorrect = 0.0;
	while(nAdjustedGestureArray[i]!= 0)
	{
		if(nDrawnSectors[j] == 0)
			break;
		
		if(nDrawnSectors[j] == nAdjustedGestureArray[i])
		{
			j++;
			i++;
		}
		else
		{
			while(nDrawnSectors[j] != 0)		// Slide!
			{
				j++;
				nNumErrors++;
				if(nDrawnSectors[j] == nAdjustedGestureArray[i])
					break;
			}
		}
	}

	// Account for unmatched overflow on the drawing
	while(nDrawnSectors[j] != 0)
	{
		j++;
		nNumErrors++;
	}

	while(nAdjustedGestureArray[i] != 0)
	{
		i++;
		nNumErrors++;
	}

	// Get Commonality Statistics
	nNumCorrectSpots = j - nNumErrors;
	if(nNumCorrectSpots < 0)
		nNumCorrectSpots = 0;
	fPercentCorrect = ((float) nNumCorrectSpots / (float) j) * (float) 100.0;

	if(fPercentCorrect < 0.0)
		fPercentCorrect = 0.0;


	// Print Results - sloppy
	char szDrawnSegs[SEG_SECTIONS * 3];
	char szGestureSegs[SEG_SECTIONS];

	j=0;
	while(nDrawnSectors[j] > 0)
	{
		szDrawnSegs[j] = nDrawnSectors[j] + '0';
		j++;
	}
	szDrawnSegs[j] = '\0';

	j=0;
	while(nAdjustedGestureArray[j] > 0)
	{
		szGestureSegs[j] = nAdjustedGestureArray[j] + '0';
		j++;
	}
	szGestureSegs[j] = '\0';

	sprintf(szResults,"Drawn:\t %s\nExpected: %s\nErrors: %d\tCorrectSpots: %d\tPercentage: %2.2f %",
		szDrawnSegs,szGestureSegs,nNumErrors,nNumCorrectSpots,fPercentCorrect );

	MessageBox(szResults,"Gesture Results");
	// Results
}


short PlaceSegment(float fXVal, float fYVal)
{
	if(fXVal <= 0.33)
	{
		if(fYVal <= 0.33)
			return 1;
		else if(fYVal <= 0.66)
			return 4;
		else
			return 7;
	}
	else if(fXVal <= 0.66)
	{
		if(fYVal <= 0.33)
			return 2;
		else if(fYVal <= 0.66)
			return 5;
		else
			return 8;
	}
	else
	{
		if(fYVal <= 0.33)
			return 3;
		else if(fYVal <= 0.66)
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
// CDrawnPoint member functions
CDrawnPoint::CDrawnPoint(CPoint point)
{
	m_Point = point;
}

void CDrawnPoint::Draw(CDC* pDC, COLORREF Color)
{
    CPen pen (PS_SOLID, 3, Color);

    CPen* pOldPen = pDC->SelectObject (&pen);
    pDC->MoveTo (m_Point);
    pDC->LineTo (m_Point);

    pDC->SelectObject (pOldPen);
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
