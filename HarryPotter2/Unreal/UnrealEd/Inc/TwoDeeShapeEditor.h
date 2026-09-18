/*=============================================================================
	TwoDeeShapeEditor : 2D Shape Editor conversion from VB code
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Warren Marshall
		* Added pieces of code from UDelaunay project
		  - arbitrary shape triangulation code
		  - support for bezier segments

    Work-in-progress todo's:
	 - "Extrude to Bevel" should extrude towards the origin point, not 0,0

=============================================================================*/

#include <stdio.h>
#include <math.h>
extern FString GLastDir[eLASTDIR_MAX];

int GGridSize = 16, GGridSizeZoom = 16;
int GDefaultDetailLevel = 5;

// *** The extra 2d utilities can be turned on/off here
#define USE_XTRA_2D_UTIL	1

#if USE_XTRA_2D_UTIL
#define DEFAULT_BITMAP_FILENAME	(TEXT("viewport.bmp"))
#define DEFAULT_BITMAP_PATH		(TEXT("..\\..\\"))
#endif

// --------------------------------------------------------------
//
// EXTRUDE Dialog
//
// --------------------------------------------------------------

int GExtrudeDepth = 256;

class WDlgExtrude : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgExtrude,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WEdit DepthEdit;

	int Depth;
 
	// Constructor.
	WDlgExtrude( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog		( TEXT("Extrude"), IDDIALOG_2DShapeEditor_Extrude, InOwnerWindow )
	,	OkButton    ( this, IDOK,     FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton( this, IDCANCEL, FDelegate(this,(TDelegate)EndDialogFalse) )
	,	DepthEdit	( this, IDEC_DEPTH )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgExtrude::OnInitDialog);
		WDialog::OnInitDialog();

		DepthEdit.SetText( *(FString::Printf(TEXT("%d"), GExtrudeDepth) ) );
		::SetFocus( DepthEdit.hWnd );

		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgExtrude::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	INT OnSetCursor()
	{
		guard(WDlgExtrude::OnSetCursor);
		WWindow::OnSetCursor();
		SetCursor(LoadCursorIdX(NULL,IDC_ARROW));
		return 0;
		unguard;
	}
	virtual int DoModal()
	{
		guard(WDlgExtrude::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgExtrude::OnOk);

		Depth = appAtoi( *DepthEdit.GetText() );

		if( Depth <= 0 )
		{
			appMsgf( TEXT("Invalid input.") );
			return;
		}

		GExtrudeDepth = Depth;
	
		EndDialogTrue();
		unguard;
	}
};

// --------------------------------------------------------------
//
// EXTRUDETOPOINT Dialog
//
// --------------------------------------------------------------

int GExtrudeToPointDepth = 256;

class WDlgExtrudeToPoint : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgExtrudeToPoint,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WEdit DepthEdit;

	int Depth;
 
	// Constructor.
	WDlgExtrudeToPoint( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog		( TEXT("ExtrudeToPoint"), IDDIALOG_2DShapeEditor_ExtrudeToPoint, InOwnerWindow )
	,	OkButton    ( this, IDOK,     FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton( this, IDCANCEL, FDelegate(this,(TDelegate)EndDialogFalse) )
	,	DepthEdit	( this, IDEC_DEPTH )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgExtrudeToPoint::OnInitDialog);
		WDialog::OnInitDialog();

		DepthEdit.SetText( *(FString::Printf(TEXT("%d"), GExtrudeToPointDepth) ) );
		::SetFocus( DepthEdit.hWnd );

		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgExtrudeToPoint::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	INT OnSetCursor()
	{
		guard(WDlgExtrudeToPoint::OnSetCursor);
		WWindow::OnSetCursor();
		SetCursor(LoadCursorIdX(NULL,IDC_ARROW));
		return 0;
		unguard;
	}
	virtual int DoModal()
	{
		guard(WDlgExtrudeToPoint::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgExtrudeToPoint::OnOk);

		Depth = appAtoi( *DepthEdit.GetText() );

		if( Depth <= 0 )
		{
			appMsgf( TEXT("Invalid input.") );
			return;
		}

		GExtrudeToPointDepth = Depth;
	
		EndDialogTrue();
		unguard;
	}
};

// --------------------------------------------------------------
//
// EXTRUDETOBEVEL Dialog
//
// --------------------------------------------------------------

int GExtrudeToBevelDepth = 128, GExtrudeToBevelCapHeight = 32;

class WDlgExtrudeToBevel : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgExtrudeToBevel,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WEdit DepthEdit, CapHeightEdit;

	int Depth, CapHeight;
 
	// Constructor.
	WDlgExtrudeToBevel( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog			( TEXT("ExtrudeToBevel"), IDDIALOG_2DShapeEditor_ExtrudeToBevel, InOwnerWindow )
	,	OkButton		( this, IDOK,     FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton	( this, IDCANCEL, FDelegate(this,(TDelegate)EndDialogFalse) )
	,	DepthEdit		( this, IDEC_DEPTH )
	,	CapHeightEdit	( this, IDEC_CAP_HEIGHT )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgExtrudeToBevel::OnInitDialog);
		WDialog::OnInitDialog();

		DepthEdit.SetText( *(FString::Printf(TEXT("%d"), GExtrudeToBevelDepth) ) );
		CapHeightEdit.SetText( *(FString::Printf(TEXT("%d"), GExtrudeToBevelCapHeight) ) );
		::SetFocus( DepthEdit.hWnd );

		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgExtrudeToBevel::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	INT OnSetCursor()
	{
		guard(WDlgExtrudeToBevel::OnSetCursor);
		WWindow::OnSetCursor();
		SetCursor(LoadCursorIdX(NULL,IDC_ARROW));
		return 0;
		unguard;
	}
	virtual int DoModal()
	{
		guard(WDlgExtrudeToBevel::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgExtrudeToBevel::OnOk);

		Depth = appAtoi( *DepthEdit.GetText() );
		CapHeight = appAtoi( *CapHeightEdit.GetText() );

		if( Depth <= 0 || CapHeight <= 0 )
		{
			appMsgf( TEXT("Invalid input.") );
			return;
		}

		GExtrudeToBevelDepth = Depth;
		GExtrudeToBevelCapHeight = CapHeight;
	
		EndDialogTrue();
		unguard;
	}
};

// --------------------------------------------------------------
//
// REVOLVE Dialog
//
// --------------------------------------------------------------

int GRevolveTotalSides = 8, GRevolveSides = 4;

class WDlgRevolve : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgRevolve,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WEdit TotalSidesEdit;
	WEdit SidesEdit;

	int TotalSides, Sides;
 
	// Constructor.
	WDlgRevolve( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog			( TEXT("Revolve"), IDDIALOG_2DShapeEditor_Revolve, InOwnerWindow )
	,	OkButton		( this, IDOK,     FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton	( this, IDCANCEL, FDelegate(this,(TDelegate)EndDialogFalse) )
	,	TotalSidesEdit	( this, IDEC_TOTAL_SIDES )
	,	SidesEdit		( this, IDEC_SIDES )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgRevolve::OnInitDialog);
		WDialog::OnInitDialog();

		TotalSidesEdit.SetText( *(FString::Printf(TEXT("%d"), GRevolveTotalSides) ) );
		::SetFocus( TotalSidesEdit.hWnd );
		SidesEdit.SetText( *(FString::Printf(TEXT("%d"), GRevolveSides) ) );

		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgRevolve::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	virtual int DoModal()
	{
		guard(WDlgRevolve::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}
	void OnOk()
	{
		guard(WDlgRevolve::OnOk);

		TotalSides = appAtoi( *TotalSidesEdit.GetText() );
		Sides = appAtoi( *SidesEdit.GetText() );

		if( TotalSides < 1
				|| Sides > TotalSides )
		{
			appMsgf( TEXT("Invalid input.") );
			return;
		}

		GRevolveTotalSides = TotalSides;
		GRevolveSides = Sides;
	
		EndDialogTrue();
		unguard;
	}
};

// --------------------------------------------------------------
//
// CUSTOM DETAIL Dialog
//
// --------------------------------------------------------------

class WDlgCustomDetail : public WDialog
{
	DECLARE_WINDOWCLASS(WDlgCustomDetail,WDialog,UnrealEd)

	WButton OKButton, CancelButton;
	WEdit ValueEdit;

	// Variables.
	int Value;

	// Constructor.
	WDlgCustomDetail( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog				( TEXT("Detail Level"), IDDIALOG_2DSE_CUSTOM_DETAIL, InOwnerWindow )
	,	CancelButton		( this, IDCANCEL, FDelegate(this,(TDelegate)EndDialogFalse) )
	,	OKButton			( this, IDOK, FDelegate(this,(TDelegate)OnOK) )
	,	ValueEdit			( this, IDEC_VALUE )
	{
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WDlgCustomDetail::OnInitDialog);
		WDialog::OnInitDialog();
		ValueEdit.SetText(TEXT("10"));
		unguard;
	}
	void OnDestroy()
	{
		guard(WDlgCustomDetail::OnDestroy);
		WDialog::OnDestroy();
		::DestroyWindow( hWnd );
		unguard;
	}
	virtual int DoModal()
	{
		guard(WDlgNewClass::DoModal);
		return WDialog::DoModal( hInstance );
		unguard;
	}

	void OnOK()
	{
		guard(WDlgCustomDetail::OnDestroy);
		Value = appAtoi( *ValueEdit.GetText() );
		EndDialogTrue();
		unguard;
	}
};

// --------------------------------------------------------------
//
// DATA TYPES
//
// --------------------------------------------------------------

class F2DSEVector : public FVector
{
public:
	F2DSEVector()
	{
		X = Y = Z = 0;
		bSelected = 0;
	}
	F2DSEVector( float x, float y, float z)
		: FVector( x, y, z )
	{
		bSelected = 0;
	}
	~F2DSEVector()
	{}

	inline F2DSEVector operator=( F2DSEVector Other )
	{
		X = Other.X;
		Y = Other.Y;
		Z = Other.Z;
		bSelected = Other.bSelected;
		return *this;
	}
	inline F2DSEVector operator=( FVector Other )
	{
		X = Other.X;
		Y = Other.Y;
		Z = Other.Z;
		return *this;
	}
	inline UBOOL operator!=( F2DSEVector Other )
	{
		return (X != Other.X && Y != Other.Y && Z != Other.Z);
	}
	inline UBOOL operator==( F2DSEVector Other )
	{
		return (X == Other.X && Y == Other.Y && Z == Other.Z);
	}
	void SelectToggle()
	{
		bSelected = !bSelected;
		if( bSelected )
			SetTempPos();
	}
	void Select( BOOL bSel )
	{
		bSelected = bSel;
		if( bSelected )
			SetTempPos();
	}
	BOOL IsSel( void )
	{
		return bSelected;
	}
	void SetTempPos()
	{
		TempX = X;
		TempY = Y;
	}

	float TempX, TempY;

private:
	UBOOL bSelected;
};

enum eSEGTYPE {
	eSEGTYPE_LINEAR	= 0,
	eSEGTYPE_BEZIER	= 1
};

// A segment represents the line drawn between 2 vertices on a shape.
class FSegment
{
public:
	FSegment()
	{
		SegType = eSEGTYPE_LINEAR;
		DetailLevel = 10;
	}
	FSegment( F2DSEVector vtx1, F2DSEVector vtx2 )
	{
		Vertex[0] = vtx1;
		Vertex[1] = vtx2;
		SegType = eSEGTYPE_LINEAR;
		DetailLevel = 10;
	}
	FSegment( FVector vtx1, FVector vtx2 )
	{
		Vertex[0] = vtx1;
		Vertex[1] = vtx2;
		SegType = eSEGTYPE_LINEAR;
		DetailLevel = 10;
	}
	~FSegment()
	{}

	inline FSegment operator=( FSegment Other )
	{
		Vertex[0] = Other.Vertex[0];
		Vertex[1] = Other.Vertex[1];
		ControlPoint[0] = Other.ControlPoint[0];
		ControlPoint[1] = Other.ControlPoint[1];
		return *this;
	}
	inline UBOOL operator==( FSegment Other )
	{
		return( Vertex[0] == Other.Vertex[0] && Vertex[1] == Other.Vertex[1] );
	}
	FVector GetHalfwayPoint()
	{
		guard(FSegment::GetHalfwayPoint);

		FVector Dir = Vertex[1] - Vertex[0];
		int Size = Dir.Size();
		Dir.Normalize();

		FVector HalfWay = Vertex[0] + (Dir * (Size / 2));
		HalfWay = HalfWay.GridSnap( FVector( GGridSize, GGridSize, GGridSize ) );
		return HalfWay;

		unguard;
	}
	void GenerateControlPoint()
	{
		guard(FSegment::GetOneThird);

		FVector Dir = Vertex[1] - Vertex[0];
		int Size = Dir.Size();
		Dir.Normalize();

		ControlPoint[0] = Vertex[0] + (Dir * (Size / 3.0f));
		ControlPoint[1] = Vertex[1] - (Dir * (Size / 3.0f));

		unguard;
	}
	void SetSegType( int InType )
	{
		guard(FSegment::SetSegType);
		if( InType == SegType ) return;
		SegType = InType;
		if( InType == eSEGTYPE_BEZIER )
		{
			GenerateControlPoint();
			DetailLevel = GDefaultDetailLevel;
		}
		unguard;
	}
	UBOOL IsSel()
	{
		guard(FSegment::IsSel);
		return ( Vertex[0].IsSel() || ControlPoint[0].IsSel() || ControlPoint[1].IsSel() );
		unguard;
	}
	void GetBezierPoints( TArray<FVector>* pBezierPoints )
	{
		guard(FSegment::IsSel);
		F2DSEVector ccp[4];
		ccp[0] = Vertex[0];
		ccp[1] = ControlPoint[0];
		ccp[2] = ControlPoint[1];
		ccp[3] = Vertex[1];
		F2DSEVector pt;

		pBezierPoints->Empty();
		(*pBezierPoints)(pBezierPoints->Add()) = Vertex[0];
		for( int n = 1 ; n <= DetailLevel ; ++n )
		{
			double nD = (double)n / (DetailLevel + 1);
			Curve( nD, ccp, pt );
			(*pBezierPoints)(pBezierPoints->Add()) = pt;
		}
		(*pBezierPoints)(pBezierPoints->Add()) = Vertex[1];
		unguard;
	}
	// Get point on cubic bezier curve (slow)
	void Curve( double nP, F2DSEVector ControlPoint[4], F2DSEVector& Position )
	{
		guard(FSegment::Curve);
		double bf[4];
		CurveBlendingFunction( nP, bf );
		Position.X = Position.Y = 0;
		for( int n = 0 ; n < 4 ; ++n )
			Position += ControlPoint[n] * bf[n];
		unguard;
	}
	// Evaluate Bernstein polynomials B_0^3(p) ... B_3^3(p)
	void CurveBlendingFunction( double nP, double pBf[4] )
	{
		guard(FSegment::CurveBlendingFunction);
		double nQ = 1 - nP;
		double nP2 = nP * nP;
		double nQ2 = nQ * nQ;
		pBf[0] = nQ2 * nQ;
		pBf[1] = 3 * nQ2 * nP;
		pBf[2] = 3 * nP2 * nQ;
		pBf[3] = nP2 * nP;
		unguard;
	}

	F2DSEVector Vertex[2], ControlPoint[2];
	int SegType,		// eSEGTYPE_
		DetailLevel;	// Detail of bezier curve
	UBOOL bUsed;
};

#define POSITION int

struct CPOLY_TREENODE
{
	int Vertex;
	POSITION NodeLeft;
	POSITION NodeRight;
};
struct CPOLY_QUEUEELEMENT
{
	int Edge;
	int Vertex;
	double Gamma;
};
struct CPOLY_VERTEX
{
	FVector XY;
	int FirstEdge;
	int EdgeCount;
};
struct CPOLY_SIDE
{
	int Vertex;
	int Triangle;
	int NextEdge;
};
struct CPOLY_EDGE
{
	CPOLY_SIDE SideLeft;
	CPOLY_SIDE SideRight;
	FVector BMin;
	FVector BMax;
};
struct STACKENTRY
{
	POSITION Node;
	int Dir;
};
struct CPOLY_TRIANGLE
{
	int Index;
	int Vertex1;
	int Vertex2;
	int Vertex3;
};
#define MAX_VERTICES 1200
#define MAX_COORD 10.0e20

// A shape is a closed series of segments (i.e. a triangle is 3 segments).
class FShape
{
public:
	FShape()
	{
	}
	~FShape()
	{}

	TArray<CPOLY_QUEUEELEMENT> m_queue;
	TArray<CPOLY_TREENODE> m_tree;
	TArray<CPOLY_VERTEX> Vertices;
	TArray<CPOLY_EDGE> Edges;
	TMap<int, CPOLY_TRIANGLE> Triangles;

	void ComputeHandlePosition()
	{
		guard(FShape::ComputeHandlePosition);
		Handle = F2DSEVector(0,0,0);
		for( int seg = 0 ; seg < Segments.Num() ; seg++ )
			Handle += Segments(seg).Vertex[0];
		Handle /= Segments.Num();
		unguard;
	}
	int AddEdge( int nVertex1, int nVertex2 )
	{
		guard(FShape::AddEdge);
		CPOLY_EDGE edge;
		CPOLY_SIDE& l = edge.SideLeft;
		CPOLY_SIDE& r = edge.SideRight;
		CPOLY_VERTEX& v1 = Vertices(nVertex1);
		CPOLY_VERTEX& v2 = Vertices(nVertex2);

		FBox bbox(v1.XY, v2.XY);
		edge.BMin = bbox.Min;
		edge.BMax = bbox.Max;

		l.Vertex = nVertex1;
		l.Triangle = -1;
		l.NextEdge = v1.FirstEdge;
		r.Vertex = nVertex2;
		r.Triangle = -1;
		r.NextEdge = v2.FirstEdge;

		int nEdge = Edges.Add();
		Edges(nEdge) = edge;

		v1.FirstEdge = nEdge;
		v1.EdgeCount++;
		v2.FirstEdge = nEdge;
		v2.EdgeCount++;

		return nEdge;
		unguard;
	}
	void InitializeBoundary( void )
	{
		guard(FShape::InitializeBoundary);
		int n, nV1, nV2, nE;
		
		int BoundaryVertexCount = Vertices.Num();

		Edges.Empty();

		for( n = 0; n < BoundaryVertexCount; ++n ) {
			Vertices(n).FirstEdge = -1;
			Vertices(n).EdgeCount = 0;
		}
		for( n = 0; n < BoundaryVertexCount; ++n ) {
			nV1 = n;
			nV2 = (n + 1) % BoundaryVertexCount;
			nE = AddEdge( nV1, nV2 );
		}
		unguard;
	}
	int FindEdge( int nVertex1, int nVertex2 )
	{	
		guard(FShape::FindEdge);
		int nFound = -1;
		int nEdge = Vertices(nVertex1).FirstEdge;
		for( int n = 0; n < Vertices(nVertex1).EdgeCount; ++n ) {
			if( nEdge < 0 )
				break;
			if( Edges(nEdge).SideLeft.Vertex == nVertex1 ) {
				if( Edges(nEdge).SideRight.Vertex == nVertex2 ) {
					nFound = nEdge;
					break;
				}
				nEdge = Edges(nEdge).SideLeft.NextEdge;
			} else {
				if( Edges(nEdge).SideLeft.Vertex == nVertex2 ) {
					nFound = nEdge;
					break;
				}
				nEdge = Edges(nEdge).SideRight.NextEdge;
			}
		}
		return nFound;
		unguard;
	}
	UBOOL FindSubTriangle( int nVertex1, int nVertex2, int nVertex3, int& nKey )
	{
		guard(FShape::FindSubTriangle);
		int nV1 = nVertex1;
		int nV2 = nVertex2;
		int nV3 = nVertex3;
		int nTemp;
		if( nV2 < nV1 ) { nTemp = nV1; nV1 = nV2; nV2 = nTemp; }
		if( nV3 < nV2 ) { nTemp = nV2; nV2 = nV3; nV3 = nTemp; }
		if( nV2 < nV1 ) { nTemp = nV1; nV1 = nV2; nV2 = nTemp; }

		nKey = (nV1 * MAX_VERTICES + nV2) * MAX_VERTICES + nV3;
		
		return (Triangles.Find( nKey ) != NULL);
		unguard;
	}
	int AddSubTriangle( int nVertex1, int nVertex2, int nVertex3 )
	{
		guard(FShape::AddSubTriangle);
		int nKey;
		UBOOL bFound = FindSubTriangle( nVertex1, nVertex2, nVertex3, nKey );
		if( !bFound ) {
			CPOLY_TRIANGLE tri;
			tri.Index = Triangles.Num();
			tri.Vertex1 = nVertex1;
			tri.Vertex2 = nVertex2;
			tri.Vertex3 = nVertex3;
			Triangles.Set( nKey, tri );
		}
		return nKey;
		unguard;
	}
	void ClearTesselation( void )
	{
		guard(FShape::ClearTesselation);
		Edges.Empty();
		Triangles.Empty();
		unguard;
	}
	// Breaks up the shape into triangles using the Delaunay triangulation method.
	void Tesselate( F2DSEVector InOrigin )
	{
		guard(FShape::Tesselate);

		ClearTesselation();

		CPOLY_TREENODE tn = { -1, -1, -1 };
		m_tree.Empty();
		POSITION posHead = m_tree.Add();
		m_tree(posHead) = tn;

		// Build a CCW list of the vertices that make up this shape.
		for( int seg = 0 ; seg < Segments.Num() ; seg++ )
			Segments(seg).bUsed = 0;

		Vertices.Empty();

		// Use up the first segment before going into the loop.
		if( Segments(0).SegType == eSEGTYPE_BEZIER )
		{
			TArray<FVector> BezierPoints;
			Segments(0).GetBezierPoints( &BezierPoints );
			for( int bz = BezierPoints.Num() - 1 ; bz > 0 ; bz-- )
			{
				new(Vertices)CPOLY_VERTEX;
				Vertices(Vertices.Num()-1).XY = BezierPoints(bz);
				Vertices(Vertices.Num()-1).FirstEdge = 0;
				Vertices(Vertices.Num()-1).EdgeCount = 0;
			}
		}
		else
		{
			new(Vertices)CPOLY_VERTEX;
			Vertices(Vertices.Num()-1).XY = Segments(0).Vertex[1];
			Vertices(Vertices.Num()-1).FirstEdge = 0;
			Vertices(Vertices.Num()-1).EdgeCount = 0;
		}
		Segments(0).bUsed = 1;

		F2DSEVector Match = Segments(0).Vertex[0];

		for( seg = 0 ; seg < Segments.Num() ; seg++ )
		{
			if( !Segments(seg).bUsed
					&& Segments(seg).Vertex[1] == Match )
			{
				if( Segments(seg).SegType == eSEGTYPE_BEZIER )
				{
					TArray<FVector> BezierPoints;
					Segments(seg).GetBezierPoints( &BezierPoints );
					for( int bz = BezierPoints.Num() - 1 ; bz > 0 ; bz-- )
					{
						new(Vertices)CPOLY_VERTEX;
						Vertices(Vertices.Num()-1).XY = BezierPoints(bz);
						Vertices(Vertices.Num()-1).FirstEdge = 0;
						Vertices(Vertices.Num()-1).EdgeCount = 0;
					}
				}
				else
				{
					new(Vertices)CPOLY_VERTEX;
					Vertices(Vertices.Num()-1).XY = Segments(seg).Vertex[1];
					Vertices(Vertices.Num()-1).FirstEdge = 0;
					Vertices(Vertices.Num()-1).EdgeCount = 0;
				}

				Segments(seg).bUsed = 1;
				Match = Segments(seg).Vertex[0];

				seg = 0;
			}
		}

		InitializeBoundary();

		int nSize1 = Vertices.Num();
		check(nSize1 > 2);

		// populate 2d tree with vertices
		static int randomizeTab[256] = {
			 32, 197, 150,  24,  44, 194, 137, 215,
			188, 253, 177, 202, 100, 165, 225, 234,
			 84, 248, 214, 235, 119,  59,   4, 176,
			 82,  37,  80, 230, 171,  55, 166, 242,
			217, 120, 236, 251, 254,  11, 174, 106,
			 34,  78,  38,  90, 140,  95, 163, 228,
			 63,  72, 212,  85, 232, 128,  92,  39,
			 13, 186, 112, 199, 193,  29,  50,  31,
			170, 167, 183, 109, 160, 168,  71,  68,
			218, 222,  20, 233,  23, 125,  25,  21,
			207,  27, 220, 238, 244,  62,  86, 127,
			115, 157,  45, 118, 206, 104, 111,  77,
			226,   5,  41,  36,  52,  47,  67,  97,
			130, 237,  76, 139, 173,  12, 164,  75,
			204,  69, 141, 246,  74, 179,  53,  46,
			 49, 159, 142, 196, 149, 203, 122,   8,
			169,  87, 102, 132,  51, 219, 156,  96,
			 58, 105, 191, 107, 146,  30, 231, 124,
			229, 138,  18, 249, 151,  28,  26,   9,
			240,  98, 135, 161,  66,  81,  83, 148,
			241,  10, 209, 136, 158, 181,  43,  73,
			154, 162, 121, 247,  57, 245,  79, 131,
			198,  93, 189, 113, 143, 182, 152,  16,
			  0, 110, 195, 129,   7, 184,  19, 243,
			190, 101, 227, 180, 210,  48,  64,  60,
			  3, 201, 250, 144, 185,  15,  94, 208,
			134,  35, 155,  54, 133, 252, 145, 205,
			116,  61, 187,  17, 211, 114,  42,   6,
			221, 123,  22, 175, 172, 216,  70, 126,
			  1, 239, 213, 153, 108, 200, 255, 192,
			  2, 223, 178,  89, 117, 103,  88, 224,
			 14,  40,  56,  65,  91, 147,  33,  99
		};

		int nSize2 = ((nSize1 - 1) | 0xFF) + 1;
		{
			for( int n = 0; n < nSize2; ++n )
			{
				int nV = (n & ~0xFF) | randomizeTab[n & 0xFF];
				if( nV < nSize1 )
					AddTreeNode( nV, posHead );
			}
		}

		// queue up initial edges of the boundary
		{ 
			for( int n = 0; n < Edges.Num(); ++n )
				QueueDelaunayEdge( n );
		}

		int nVert1, nVert2, nVert3, nEdge1, nEdge2, nEdge3;
		int nTriangle;
		UBOOL bFree;

		while( m_queue.Num() )
		{
			CPOLY_QUEUEELEMENT qe = m_queue(0);
			m_queue.Remove(0);
			nEdge1 = qe.Edge;
			nVert3 = qe.Vertex;

			CPOLY_EDGE& edge1 = Edges(nEdge1);
			bFree = (edge1.SideLeft.Triangle < 0);
			nVert1 = edge1.SideLeft.Vertex;
			nVert2 = edge1.SideRight.Vertex;

			if( bFree )
			{
				nEdge2 = FindEdge( nVert3, nVert2 );
				nEdge3 = FindEdge( nVert1, nVert3 );

				// add new triangle
				nTriangle = AddSubTriangle( nVert1, nVert2, nVert3 );

				// got two new edges, queue them too
				Edges(nEdge1).SideLeft.Triangle = nTriangle;
				
				if( nEdge2 < 0 )
				{
					nEdge2 = AddEdge( nVert3, nVert2 );
					CPOLY_EDGE& edge2 = Edges(nEdge2);
					edge2.SideRight.Triangle = nTriangle;
					QueueDelaunayEdge( nEdge2 );
				}
				else
				{
					CPOLY_EDGE& edge2 = Edges(nEdge2);
					if( edge2.SideLeft.Triangle == -1 )
						edge2.SideLeft.Triangle = nTriangle;
					else
					{
						if( edge2.SideRight.Triangle == -1 )
							edge2.SideRight.Triangle = nTriangle;
					}
				}

				if( nEdge3 < 0 )
				{
					nEdge3 = AddEdge( nVert1, nVert3 );
					CPOLY_EDGE& edge3 = Edges(nEdge3);
					edge3.SideRight.Triangle = nTriangle;
					QueueDelaunayEdge( nEdge3 );
				}
				else
				{
					CPOLY_EDGE& edge3 = Edges(nEdge3);
					if( edge3.SideLeft.Triangle == -1 )
						edge3.SideLeft.Triangle = nTriangle;
					else
					{
						if( edge3.SideRight.Triangle == -1 )
							edge3.SideRight.Triangle = nTriangle;
					}
				}
			}
		}

		//
		// Massage the vertices/triangles before leaving ...
		//

		for( int vertex = 0 ; vertex < Vertices.Num() ; vertex++ )
		{
			Vertices(vertex).XY -= InOrigin;
		}

		unguard;
	}
	POSITION AddTreeNode( int nVertex, POSITION posNode )
	{
		guard(FShape::AddTreeNode);
		check( posNode != -1 );

		CPOLY_TREENODE* pNode;
		CPOLY_TREENODE* pParent;
		POSITION pos = posNode;
		FVector& pt = Vertices(nVertex).XY;
		UBOOL bResult;
		int nV;
		int nDir = 0;

		// NOTE: tree is not balanced (the extra effort would be overkill
		//  for such small vertex sets.  Splay trees are cool, though.)

		do {
			pNode = &m_tree( pos );
			pParent = pNode;
			nV = pNode->Vertex;

			if( nV < 0 )
				bResult = 0;
			else
				if( nDir)
					bResult = (pt.Y < Vertices(nV).XY.Y);
				else
					bResult = (pt.X < Vertices(nV).XY.X);

			if( bResult )
				pos = pNode->NodeLeft;
			else
				pos = pNode->NodeRight;

			nDir = (nDir + 1) & 1;
		} while( pos != -1 );
		
		CPOLY_TREENODE tn;
		tn.Vertex = nVertex;
		tn.NodeLeft = -1;
		tn.NodeRight = -1;
		pos = m_tree.Add();
		m_tree(pos) = tn;
		
		if( bResult )
			pParent->NodeLeft = pos;
		else
			pParent->NodeRight = pos;

		return pos;
		unguard;
	}
	// Gets orientation of a triangle (pt1, pt2, pt3).  Return values are:
	//         +2: counter-clockwise
	//         +1: collinear, pt3 between pt1 and pt2
	//          0: collinear, pt1 between pt2 and pt3
	//         -1: collinear, pt2 between pt1 and pt3
	//         -2: clockwise
	//
	int Orientation( const FVector& pt1,
					const FVector& pt2,
					const FVector& pt3 )
	{
		guard(FShape::Orientation);
		FVector v1 = pt2 - pt1;
		FVector v2 = pt3 - pt1;
		double d = v1.X * v2.Y - v1.Y * v2.X;

		if( fabs( d ) < 0.000001 ) {
			if( (v1 | v2 ) < 0 )
				return 0;
			else if( v1.Size() > v2.Size() )
				return -1;
			else
				return 1;
		} 
		else
		{
			if( d < 0 )
				return -2;
			else
				return 2;
		}
		unguard;
	}
	// Tests for any intersections of existing edges and the line segment
	//  between nV1 and nV2.
	bool TestEdgeIntersection( int nV1, int nV2 )
	{
		guard(FShape::TestEdgeIntersection);
		bool bIntersect = false;

		FVector p1 = Vertices(nV1).XY;
		FVector p2 = Vertices(nV2).XY;
		FBox bbox(p1, p2);
		FVector ptBMin = bbox.Min;
		FVector ptBMax = bbox.Max;

		// Linear search. Running time could be cut down by storing
		//  the edge data in a spatial R-Tree
		for( int n = 0; n < Edges.Num(); ++n )
		{
			CPOLY_EDGE& edge = Edges(n);
			if( ptBMin.X <= edge.BMax.X && ptBMin.Y <= edge.BMax.Y
					&& ptBMax.X >= edge.BMin.X && ptBMax.Y >= edge.BMin.Y )
			{
				int nV3 = edge.SideLeft.Vertex;
				int nV4 = edge.SideRight.Vertex;

				if( nV3 != nV1 && nV3 != nV2 && nV4 != nV1 && nV4 != nV2 )
				{
					FVector& q1 = Vertices(nV3).XY;
					FVector& q2 = Vertices(nV4).XY;
					int o1, o2, o3, o4;
					o1 = Orientation( p1, q1, q2 );
					o2 = Orientation( p2, q1, q2 );
					o3 = Orientation( q1, p2, p1 );
					o4 = Orientation( q2, p2, p1 );

					if( o1 * o2 <= 0 && o3 * o4 <= 0 )
					{
						bIntersect = true;
						break;
					}
				}
			}
		}

		return bIntersect;
		unguard;
	}
	// QueueDelaunayEdge searches a third vertex satisfying the Delaunay
	//  criterion for the given edge, and stores the search result in a queue.
	void QueueDelaunayEdge( int nEdge )
	{
		guard(FShape::QueueDelaunayEdge);
		static TArray<STACKENTRY> stack;
		STACKENTRY se;

		FVector va, vb, vd, pc;
		int nVertex = 0;
		double a0, b0, d0, d1, d2, r;
		UBOOL bInsideRect, b1, b2;

		int nV1 = Edges(nEdge).SideLeft.Vertex;
		int nV2 = Edges(nEdge).SideRight.Vertex;
		int nV3 = 0;
		FVector& p1 = Vertices(nV1).XY;
		FVector& p2 = Vertices(nV2).XY;
		FVector p3;

		FVector vc = p2 - p1;

		double c0 = vc.SizeSquared();
		double cosC = 0;
		double cosCmin = 0;
		UBOOL bFound = 0;
		UBOOL bIntersectAll = 0;
		UBOOL bIntersect;

		while( !bFound )
		{
			stack.Empty();

			FVector rectMin( -MAX_COORD, -MAX_COORD, 0 );
			FVector rectMax( MAX_COORD, MAX_COORD, 0 );
			int nDir = 0;
			POSITION posNode = 0;//m_tree.GetHeadPosition();

			cosC = 0;
			cosCmin = 2;

			int nReturn = 0; // Return "address"

			for( ;; )
			{
				while( posNode != -1 )
				{
					// 2D tree range searching (Sedgewick: Algorithms)
					nV3 = m_tree( posNode ).Vertex;

					// LABEL 0
					if( nReturn < 1 )
					{
						if( nV3 < 0 )
							bInsideRect = 0;
						else
						{
							p3 = Vertices(nV3).XY;
							bInsideRect =
								(p3.X >= rectMin.X
								&& p3.Y >= rectMin.Y
								&& p3.X <= rectMax.X
								&& p3.Y <= rectMax.Y);
						}

						if( bInsideRect )
						{
							if( nV3 != nV1 && nV3 != nV2 )
							{
								if( Orientation( p1, p2, p3 ) == 2 )
								{
									va = p2 - p3;
									vb = p3 - p1;
									a0 = va.SizeSquared();
									b0 = vb.SizeSquared();
									// Delaunay criterion
									cosC = (a0 + b0 - c0) / sqrt( a0 * b0 );
									if( cosC < cosCmin )
									{
										if( bIntersectAll )
											bIntersect = TestEdgeIntersection( nV3, nV2 ) || TestEdgeIntersection( nV1, nV3 );
										else
											bIntersect = 0;

										if( !bIntersect )
										{
											bFound = 1;
											nVertex = nV3;
											cosCmin = cosC;
											d0 = vc.X * (p1.X + p2.X) + vc.Y * (p1.Y + p2.Y);
											d1 = vb.X * (p3.X + p1.X) + vb.Y * (p3.Y + p1.Y);
											d2 = 2 * (vc.X * va.Y - vc.Y * va.X);
											pc = FVector(
												(vc.Y * d1 - vb.Y * d0) / d2,
												(vb.X * d0 - vc.X * d1) / d2,
												0);
											vd = p1 - pc;
											r = vd.Size() * 1.01;
											FBox bbox( rectMin, rectMax );
											bbox += FVector( pc.X - r, pc.Y - r, 0);
											bbox += FVector( pc.X + r, pc.Y + r, 0);
											rectMin = bbox.Min;
											rectMax = bbox.Max;
										}
									}
								}
							}
						}
						
						if( nV3 < 0 )
							b1 = 0;
						else
						{
							p3 = Vertices(nV3).XY;
							b1 = (nDir == 0) ? (rectMin.X < p3.X) : (rectMin.Y < p3.Y);
						}

						if( b1 )
						{
							se.Node = posNode;
							se.Dir = nDir;
							stack(stack.Add()) = se;
							posNode = m_tree( posNode ).NodeLeft;
							nDir = (nDir + 1) & 1;
							nReturn = 2; // left branch
						}
					}

					// LABEL 1
					if( nReturn < 2 )
					{
						if( nV3 < 0 )
							b2 = 1;
						else
						{
							p3 = Vertices(nV3).XY;
							b2 = (nDir == 0) ? (rectMax.X >= p3.X) : (rectMax.Y >= p3.Y);
						}

						if( b2 )
						{
							// right branch
							posNode = m_tree( posNode ).NodeRight;
							nDir = (nDir + 1) & 1;
						}
						else
							break;

					}
					nReturn = 0;
				}

				if( stack.Num() <= 0 ) break;
				posNode = stack(stack.Num()-1).Node;
				nDir = stack(stack.Num()-1).Dir;
				stack.Remove( stack.Num()-1 );
				nReturn = 1;
			}

			if( !bFound )
				if( bIntersectAll )
					break;
			else 
				if( !bIntersectAll )
				{
					bIntersect = TestEdgeIntersection( nVertex, nV2 ) || TestEdgeIntersection( nV1, nVertex );
					if( bIntersect )
						bFound = 0;
					else
						break;
				}

			bIntersectAll = 1;
		}

		if( bFound )
		{
			CPOLY_QUEUEELEMENT qe;
			qe.Edge = nEdge;
			qe.Vertex = nVertex;
			qe.Gamma = cosCmin;
			for( int x = 0 ; x < m_queue.Num() ; x++ )
				if( m_queue( x ).Gamma > cosCmin )
					break;

			if( x < m_queue.Num() )
			{
				m_queue.Insert(x);
				m_queue(x) = qe;
			}
			else
				m_queue(m_queue.Add()) = qe;
		}
		unguard;
	}

	F2DSEVector Handle;
	TArray<FSegment> Segments;
};

#define d2dSE_SELECT_TOLERANCE 4

// --------------------------------------------------------------
//
// W2DSHAPEEDITOR
//
// --------------------------------------------------------------

#define ID_2DSE_TOOLBAR	29002
TBBUTTON tb2DSEButtons[] = {
	{ 0, IDMN_2DSE_NEW, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 1, IDMN_2DSE_FileOpen, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 2, IDMN_2DSE_FileSave, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 3, IDMN_2DSE_ROTATE90, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 4, IDMN_2DSE_ROTATE45, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 5, IDMN_2DSE_FLIP_VERT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 6, IDMN_2DSE_FLIP_HORIZ, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 7, IDMN_2DSE_SCALE_UP, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 8, IDMN_2DSE_SCALE_DOWN, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 9, IDMN_2DSE_ZOOM_IN, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 10, IDMN_2DSE_ZOOM_OUT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 11, IDMN_2DSE_SPLIT_SIDE, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 12, IDMN_2DSE_DELETE, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 13, IDMN_SEGMENT_LINEAR, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 14, IDMN_SEGMENT_BEZIER, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 0, 0, TBSTATE_ENABLED, TBSTYLE_SEP, 0L, 0}
	, { 15, IDMN_2DSE_PROCESS_SHEET, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 16, IDMN_2DSE_PROCESS_REVOLVE, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 17, IDMN_2DSE_PROCESS_EXTRUDE, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 18, IDMN_2DSE_PROCESS_EXTRUDETOPOINT, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
	, { 19, IDMN_2DSE_PROCESS_EXTRUDETOBEVEL, TBSTATE_ENABLED, TBSTYLE_BUTTON, 0L, 0}
};
struct {
	TCHAR ToolTip[64];
	int ID;
} ToolTips_2DSE[] = {
	TEXT("New"), IDMN_2DSE_NEW,
	TEXT("Open"), IDMN_2DSE_FileOpen,
	TEXT("Save"), IDMN_2DSE_FileSave,
	TEXT("Rotate 90 Degrees"), IDMN_2DSE_ROTATE90,
	TEXT("Rotate 45 Degrees"), IDMN_2DSE_ROTATE45,
	TEXT("Flip Vertically"), IDMN_2DSE_FLIP_VERT,
	TEXT("Flip Horizontally"), IDMN_2DSE_FLIP_HORIZ,
	TEXT("Create a Sheet"), IDMN_2DSE_PROCESS_SHEET,
	TEXT("Extruded Shape"), IDMN_2DSE_PROCESS_EXTRUDE,
	TEXT("Revolved Shape"), IDMN_2DSE_PROCESS_REVOLVE,
	TEXT("Extude to Point"), IDMN_2DSE_PROCESS_EXTRUDETOPOINT,
	TEXT("Extude to Bevel"), IDMN_2DSE_PROCESS_EXTRUDETOBEVEL,
	TEXT("Scale Up"), IDMN_2DSE_SCALE_UP,
	TEXT("Scale Down"), IDMN_2DSE_SCALE_DOWN,
	TEXT("Split Segment(s)"), IDMN_2DSE_SPLIT_SIDE,
	TEXT("Delete"), IDMN_2DSE_DELETE,
	TEXT("Linear Segment"), IDMN_SEGMENT_LINEAR,
	TEXT("Bezier Segment"), IDMN_SEGMENT_BEZIER,
	TEXT("Zoom In"), IDMN_2DSE_ZOOM_IN,
	TEXT("Zoom Out"), IDMN_2DSE_ZOOM_OUT,
	NULL, 0
};

class W2DShapeEditor : public WWindow
{
	DECLARE_WINDOWCLASS(W2DShapeEditor,WWindow,Window)

	// Structors.
	W2DShapeEditor( FName InPersistentName, WWindow* InOwnerWindow )
	:	WWindow( InPersistentName, InOwnerWindow )
	{
		New();
		m_bDraggingCamera = m_bDraggingVerts = FALSE;
		::ZeroMemory( MapFilename, sizeof(TCHAR) * 256);
		hImage = NULL;
		hWndToolBar = NULL;
		Zoom = 1.0f;
	}

	WToolTip* ToolTipCtrl;
	HWND hWndToolBar;
	FVector m_camera;			// The viewing camera position
	F2DSEVector m_origin;		// The origin point used for revolves and such
	BOOL m_bDraggingCamera, m_bDraggingVerts, m_bMouseHasMoved;
	FPoint m_pointOldPos;
	TCHAR MapFilename[256];
	HBITMAP hImage;
	RECT m_rcWnd;
	POINT m_ContextPos;
	float Zoom;

	TArray<FShape> m_shapes;

	// WWindow interface.
	void OpenWindow()
	{
		guard(W2DShapeEditor::OpenWindow);
		MdiChild = 0;

		PerformCreateWindowEx
		(
			0,
			NULL,
			WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
			0,
			0,
			320,
			200,
			OwnerWindow ? OwnerWindow->hWnd : NULL,
			NULL,
			hInstance
		);
		SetCaption();
		unguard;
	}
	void OnDestroy()
	{
		guard(W2DShapeEditor::OnDestroy);
		delete ToolTipCtrl;
		::DestroyWindow( hWndToolBar );
		WWindow::OnDestroy();
		unguard;
	}
	INT OnSetCursor()
	{
		guard(W2DShapeEditor::OnSetCursor);
		WWindow::OnSetCursor();
		SetCursor(LoadCursorIdX(NULL,IDC_CROSS));
		return 0;
		unguard;
	}
	void OnCreate()
	{
		guard(W2DShapeEditor::OnCreate);
		WWindow::OnCreate();

		SetMenu( hWnd, LoadMenuIdX(hInstance, IDMENU_2DShapeEditor) );

		// TOOLBAR
		HWND hWndToolBar = CreateToolbarEx( 
			hWnd,
			WS_CHILD | WS_BORDER | WS_VISIBLE | CCS_ADJUSTABLE,
			ID_2DSE_TOOLBAR,
			20,
			hInstance,
			IDB_2DSE_TOOLBAR,
			(LPCTBBUTTON)&tb2DSEButtons,
			27,
			18,18,
			18,18,
			sizeof(TBBUTTON));

		if( !hWndToolBar )
			appMsgf( TEXT("Toolbar not created!") );

		ToolTipCtrl = new WToolTip(this);
		ToolTipCtrl->OpenWindow();

		for( int tooltip = 0 ; ToolTips_2DSE[tooltip].ID > 0 ; tooltip++ )
		{
			int index = SendMessageX( hWndToolBar, TB_COMMANDTOINDEX, ToolTips_2DSE[tooltip].ID, 0 );
			RECT rect;
			SendMessageX( hWndToolBar, TB_GETITEMRECT, index, (LPARAM)&rect);

			ToolTipCtrl->AddTool( hWndToolBar, ToolTips_2DSE[tooltip].ToolTip, tooltip, &rect );
		}

		unguard;
	}
	void OnPaint()
	{
		guard(W2DShapeEditor::OnPaint);
		PAINTSTRUCT PS;
		HDC hDC = BeginPaint( *this, &PS );

		HDC hdcWnd, hdcMem;
		HBITMAP hBitmap;
		HBRUSH l_brush, l_brushOld = NULL;

		::GetClientRect( hWnd, &m_rcWnd );

		hdcWnd = GetDC(hWnd);
		hdcMem = CreateCompatibleDC(hdcWnd);
		hBitmap = CreateCompatibleBitmap(hdcWnd, m_rcWnd.right, m_rcWnd.bottom );
		SelectObject(hdcMem, hBitmap);

		l_brush = CreateSolidBrush( RGB(255, 255, 255) );
		SelectObject( hdcMem, l_brush);
		FillRect( hdcMem, GetClientRect(), l_brush );
		l_brushOld = (HBRUSH)SelectObject( hdcMem, l_brushOld);

		DrawGrid( hdcMem );
		DrawImage( hdcMem );
		DrawOrigin( hdcMem );
		DrawSegments( hdcMem );
		DrawShapeHandles( hdcMem );
		DrawVertices( hdcMem );

		BitBlt(hDC,
			   0, 0,
			   m_rcWnd.right, m_rcWnd.bottom,
			   hdcMem,
			   0, 0,
			   SRCCOPY);

		EndPaint( *this, &PS );

		DeleteObject( l_brush );
		DeleteDC(hdcMem);
		ReleaseDC( hWnd, hdcWnd );
		DeleteObject( hBitmap );
		unguard;
	}
	void ScaleShapes( float InScale )
	{
		guard(W2DShapeEditor::ScaleShapes);

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
			{
				for( int vertex = 0 ; vertex < 2 ; vertex++ )
				{
					m_shapes(shape).Segments(seg).Vertex[vertex].X *= InScale;
					m_shapes(shape).Segments(seg).Vertex[vertex].Y *= InScale;
					m_shapes(shape).Segments(seg).ControlPoint[vertex].X *= InScale;
					m_shapes(shape).Segments(seg).ControlPoint[vertex].Y *= InScale;
				}
			}
		}

		ComputeHandlePositions(1);
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}




#if USE_XTRA_2D_UTIL

	void Util_SetClient( HWND hWnd, int iWidth, int iHeight )
	{
		RECT	rc = { 0, 0, iWidth, iHeight };
		int		iRealWidth, iRealHeight;
		
		// Adjust our window rect for the desired client window.
		::AdjustWindowRect( &rc, GetWindowLong(hWnd, GWL_STYLE), true );
		
		// AdjustWindowRect will tell us the real width and height of our window. (*including boarders*)
		iRealWidth	= abs( rc.left ) + rc.right;
		iRealHeight	= abs( rc.top  ) + rc.bottom + 31; // <- add 31 pixels for the toolbar under the menu
		
		// get the current position of the window through "GetWindowRect"
		::GetWindowRect(hWnd, &rc);

		// We can make sure our client window is correct now that we know the real width and height
		::MoveWindow(hWnd, rc.left, rc.top, iRealWidth, iRealHeight, true );
	}

	bool Util_SaveClientBMP( HWND hWnd, int Channels )
	{
		CBitmap bmp;

		// error checking
		if( hWnd == 0 )
			return false;

		RECT rcClient;
		RECT rcWindow;
		int	 iXOffset, iYOffset;

		// Find our x and y offset from the position of the main window to our client space
		::GetClientRect(hWnd, &rcClient);
		::AdjustWindowRect( &rcClient, GetWindowLong(hWnd, GWL_STYLE), true );
		
		iXOffset = abs( rcClient.left );
		iYOffset = abs( rcClient.top  ) + 31; // <- add 31 pixels for the toolbar under the menu
		
		// Get our window rect for position of window and our client rect for size of client space
		::GetWindowRect(hWnd, &rcWindow);
		::GetClientRect(hWnd, &rcClient);
		
		// Now that we have our position and size data lets create our bitmap and save it.
		if( bmp.CreateFromDesktop( rcWindow.left + iXOffset, 
								   rcWindow.top  + iYOffset, 
								   rcClient.right, 
								   rcClient.bottom-31, // subtract 31 pixels for the toolbar under the menu
								   Channels ) )
		{
			// Get the filename to save this as
			
			// Handle the open file command
			TCHAR	strFinalFilename[512];
			TCHAR	strTitleFilename[512];
			
			//set default filename
			appStrcpy(strFinalFilename, DEFAULT_BITMAP_FILENAME );

			// Display the OpenFileName dialog. Then, try to load the specified file
			OPENFILENAME ofn = 
			{ 
				sizeof(OPENFILENAME),				// DWORD         lStructSize;
				NULL,								// HWND          hwndOwner;
				NULL,								// HINSTANCE     hInstance;
				TEXT(".bmp files (.bmp)\0*.bmp\0\0"),// LPCWSTR      lpstrFilter;
				NULL,								// LPWSTR        lpstrCustomFilter;
				0,									// DWORD         nMaxCustFilter;
				1,									// DWORD         nFilterIndex;
				strFinalFilename,					// LPWSTR        lpstrFile;			//initial value for filename in dialog box
				512,								// DWORD         nMaxFile;
				strTitleFilename,					// LPWSTR        lpstrFileTitle;		//title of filename with ext. but no path
				512,								// DWORD         nMaxFileTitle;
				NULL,//DEFAULT_BITMAP_PATH,				// LPCWSTR       lpstrInitialDir;
				TEXT("Save Bitmap"),				// LPCWSTR       lpstrTitle;
				OFN_EXPLORER,						// DWORD         Flags;
				0,									// WORD          nFileOffset;
				1,									// WORD          nFileExtension;
				TEXT("bmp"),						// LPCWSTR       lpstrDefExt;
				0,									// LPARAM        lCustData;
				NULL,								// LPOFNHOOKPROC lpfnHook;
				NULL };								// LPCWSTR       lpTemplateName;
			
			if( GetSaveFileName( &ofn ) )
			{	
				// We successfully created a bitmap from the client window so lets save it			
				// convert our TCHAR to ansi so we can pass it into our SaveBitmap Function
				bmp.SaveFile( TCHAR_TO_ANSI(strFinalFilename) );
			}

			// Clean up
			bmp.Destroy();
			return true; // success
		}
		return false; // failure
	}

#endif

	void OnCommand( INT Command )
	{
		guard(W2DShapeEditor::OnCommand);
		switch( Command ) {

			case ID_FileExit:
				SendMessageX( hWnd, WM_CLOSE, 0, 0 );
				break;

			case IDMN_2DSE_ZOOM_IN:

				Zoom *= 2.0f;
				if( Zoom > 8.0f) Zoom = 8.0f;
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;

			case IDMN_2DSE_ZOOM_OUT:
				Zoom *= 0.5f;
				if( Zoom < 0.125f) Zoom = 0.125f;
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;

#if USE_XTRA_2D_UTIL		
			case IDMN_2DSE_PER_PIXEL_AT_256X256:
				
				// make our zoom a 1 : 1 ratio to pixels
				Zoom		  = 1.0f;
				GGridSizeZoom = GGridSize * Zoom;
				
				// make our client window 256x256 
				Util_SetClient( hWnd, 256, 256 );

				// The camera's Y center position is farther down because
				// we need to make up for the space taken by the toolbar.
				m_camera.X = 0;
				m_camera.Y = 16;
				
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			
			case IDMN_SAVE_VIEWPORT_AS_BMP:			
				Util_SaveClientBMP( hWnd, 3 );
				break;
#endif


			case IDMN_2DSE_SCALE_UP:
				ScaleShapes(2.0f);
				break;

			case IDMN_2DSE_SCALE_DOWN:
				ScaleShapes(0.5f);
				break;

			case IDMN_2DSE_ROTATE90:
				RotateShape( 90 );
				break;

			case IDMN_2DSE_ROTATE45:
				RotateShape( 45 );
				break;

			case IDMN_DETAIL_1:
				SetSegmentDetail(1);
				break;

			case IDMN_DETAIL_2:
				SetSegmentDetail(2);
				break;

			case IDMN_DETAIL_3:
				SetSegmentDetail(3);
				break;

			case IDMN_DETAIL_4:
				SetSegmentDetail(4);
				break;

			case IDMN_DETAIL_5:
				SetSegmentDetail(5);
				break;

			case IDMN_DETAIL_10:
				SetSegmentDetail(10);
				break;

			case IDMN_DETAIL_15:
				SetSegmentDetail(15);
				break;

			case IDMN_DETAIL_20:
				SetSegmentDetail(20);
				break;

			case IDMN_DETAIL_CUSTOM:
				{
					WDlgCustomDetail dlg( NULL, this );
					if( dlg.DoModal() )
						SetSegmentDetail( dlg.Value );
				}
				break;

			case IDMN_2DSE_FLIP_VERT:
				Flip(0);
				break;

			case IDMN_2DSE_FLIP_HORIZ:
				Flip(1);
				break;

			case IDMN_SEGMENT_LINEAR:
				ChangeSegmentTypes(eSEGTYPE_LINEAR);
				break;

			case IDMN_SEGMENT_BEZIER:
				ChangeSegmentTypes(eSEGTYPE_BEZIER);
				break;

			case IDMN_2DSE_NEW:
				appStrcpy( MapFilename, TEXT("") );
				SetCaption();
				New();
				break;

			case IDMN_2DSE_SPLIT_SIDE:
				SplitSides();
				break;

			case IDMN_2DSE_NEW_SHAPE:
				InsertNewShape();
				break;

			case IDMN_2DSE_DELETE:
				Delete();
				break;

			case IDMN_2DSE_FileSave:
				FileSave( hWnd );
				break;

			case IDMN_2DSE_FileSaveAs:
				FileSaveAs( hWnd );
				break;

			case IDMN_2DSE_FileOpen:
				FileOpen( hWnd );
				break;

			case IDMN_2DSEC_SET_ORIGIN:
				SetOrigin();
				break;

			case IDMN_2DSE_PROCESS_SHEET:
				ProcessSheet();
				break;

			case IDMN_2DSE_PROCESS_EXTRUDE:
				{
					WDlgExtrude Dialog( NULL, this );
					if( Dialog.DoModal() == IDOK )
						ProcessExtrude( Dialog.Depth );
				}
				break;

			case IDMN_2DSE_PROCESS_EXTRUDETOPOINT:
				{
					WDlgExtrudeToPoint Dialog( NULL, this );
					if( Dialog.DoModal() == IDOK )
						ProcessExtrudeToPoint( Dialog.Depth );
				}
				break;

			case IDMN_2DSE_PROCESS_EXTRUDETOBEVEL:
				{
					WDlgExtrudeToBevel Dialog( NULL, this );
					if( Dialog.DoModal() == IDOK )
						ProcessExtrudeToBevel( Dialog.Depth, Dialog.CapHeight );
				}
				break;

			case IDMN_2DSE_PROCESS_REVOLVE:
				{
					WDlgRevolve Dialog( NULL, this );
					if( Dialog.DoModal() == IDOK )
						ProcessRevolve( Dialog.TotalSides, Dialog.Sides );
				}
				break;

			case IDMN_GRID_1:	
				GGridSize = 1;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			case IDMN_GRID_2:	
				GGridSize = 2;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			case IDMN_GRID_4:	
				GGridSize = 4;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			case IDMN_GRID_8:	
				GGridSize = 8;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			case IDMN_GRID_16:	
				GGridSize = 16;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			case IDMN_GRID_32:	
				GGridSize = 32;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;
			case IDMN_GRID_64:	
				GGridSize = 64;	
				GGridSizeZoom = GGridSize * Zoom;
				InvalidateRect( hWnd, NULL, FALSE );
				break;

			case IDMN_2DSE_GET_IMAGE:
				{
					if( !GEditor->CurrentTexture )
					{
						appMsgf(TEXT("Select a texture in the browser first."));
						return;
					}

					BITMAPINFO bmi;
					BYTE* pBits = NULL;

					::ZeroMemory( &bmi, sizeof(BITMAPINFOHEADER) );
					bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
					bmi.bmiHeader.biWidth = GEditor->CurrentTexture->USize;
					bmi.bmiHeader.biHeight = -GEditor->CurrentTexture->VSize;
					bmi.bmiHeader.biPlanes = 1;
					bmi.bmiHeader.biBitCount = 24;
					bmi.bmiHeader.biCompression = BI_RGB;

					if( hImage ) 
						DeleteObject( hImage );
					hImage = CreateDIBSection( ::GetDC( hWnd ), &bmi, DIB_RGB_COLORS, (void**)&pBits, NULL, 0 );
					check( hImage );
					check( pBits );

					// RGB DATA
					FColor* Palette = GEditor->CurrentTexture->GetColors();
					check(Palette);
					BYTE* pSrc = GEditor->CurrentTexture->GetMip(0)->DataPtr;
					BYTE* pDst = pBits;
					for( int x = 0 ; x < GEditor->CurrentTexture->USize * GEditor->CurrentTexture->VSize; x++ )
					{
						FColor color = Palette[ *pSrc ];
						*pDst = color.B;	pDst++;
						*pDst = color.G;	pDst++;
						*pDst = color.R;	pDst++;
						pSrc++;
					}
				}
				break;

			case IDMN_2DSE_OPEN_IMAGE:
				{
					OPENFILENAMEA ofn;
					char File[8192] = "\0";

					ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
					ofn.lStructSize = sizeof(OPENFILENAMEA);
					ofn.hwndOwner = hWnd;
					ofn.lpstrFile = File;
					ofn.nMaxFile = sizeof(char) * 8192;
					ofn.lpstrFilter = "Bitmaps (*.bmp)\0*.bmp\0All Files\0*.*\0\0";
					ofn.lpstrInitialDir = "..\\maps";
					ofn.lpstrTitle = "Open Image";
					ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UTX]) );
					ofn.lpstrDefExt = "bmp";
					ofn.Flags = OFN_NOCHANGEDIR;

					// Display the Open dialog box. 
					//
					if( GetOpenFileNameA(&ofn) )
					{
						if( hImage ) 
							DeleteObject( hImage );

						hImage = (HBITMAP)LoadImageA( hInstance, File, IMAGE_BITMAP, 0, 0, LR_LOADFROMFILE );

						if( !hImage )
							appMsgf ( TEXT("Error loading bitmap.") );

						FString S = appFromAnsi( File );
						GLastDir[eLASTDIR_UTX] = S.Left( S.InStr( TEXT("\\"), 1 ) );
					}

					InvalidateRect( hWnd, NULL, FALSE );
				}
				break;

			case IDMN_2DSE_DELETE_IMAGE:
				DeleteObject( hImage );
				hImage = NULL;
				InvalidateRect( hWnd, NULL, FALSE );
				break;

			default:
				WWindow::OnCommand(Command);
				break;
		}
		unguard;
	}
	void OnRightButtonDown()
	{
		guard(W2DShapeEditor::OnRightButtonDown);

		m_bDraggingCamera = TRUE;
		m_pointOldPos = GetCursorPos();
		SetCapture( hWnd );
		m_bMouseHasMoved = FALSE;

		WWindow::OnRightButtonDown();
		unguard;
	}
	void OnRightButtonUp()
	{
		guard(W2DShapeEditor::OnRightButtonUp);

		ReleaseCapture();
		m_bDraggingCamera = FALSE;

		if( !m_bMouseHasMoved )
		{
			::GetCursorPos( &m_ContextPos );
			HMENU l_menu = GetSubMenu( LoadMenuIdX(hInstance, IDMENU_2DShapeEditor_Context), 0 );

			CheckMenuItem( l_menu, IDMN_GRID_1, MF_BYCOMMAND | ((GGridSize == 1) ? MF_CHECKED : MF_UNCHECKED) );
			CheckMenuItem( l_menu, IDMN_GRID_2, MF_BYCOMMAND | ((GGridSize == 2) ? MF_CHECKED : MF_UNCHECKED) );
			CheckMenuItem( l_menu, IDMN_GRID_4, MF_BYCOMMAND | ((GGridSize == 4) ? MF_CHECKED : MF_UNCHECKED) );
			CheckMenuItem( l_menu, IDMN_GRID_8, MF_BYCOMMAND | ((GGridSize == 8) ? MF_CHECKED : MF_UNCHECKED) );
			CheckMenuItem( l_menu, IDMN_GRID_16, MF_BYCOMMAND | ((GGridSize == 16) ? MF_CHECKED : MF_UNCHECKED) );
			CheckMenuItem( l_menu, IDMN_GRID_32, MF_BYCOMMAND | ((GGridSize == 32) ? MF_CHECKED : MF_UNCHECKED) );
			CheckMenuItem( l_menu, IDMN_GRID_64, MF_BYCOMMAND | ((GGridSize == 64) ? MF_CHECKED : MF_UNCHECKED) );

			TrackPopupMenu( l_menu,
				TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
				m_ContextPos.x, m_ContextPos.y, 0,
				hWnd, NULL);

			::ScreenToClient( hWnd, &m_ContextPos );
		}

		WWindow::OnRightButtonUp();
		unguard;
	}
	void OnMouseMove( DWORD Flags, FPoint MouseLocation )
	{
		guard(W2DShapeEditor::OnMouseMove);

		m_bMouseHasMoved = TRUE;

		float pct = 1.0f;
		if( Zoom > 2.0f )
			pct = 0.5f;
		else if( Zoom < 0.5f )
			pct = 4.0f;

		POINT Delta;
		Delta.x = (MouseLocation.X - m_pointOldPos.X) * pct;
		Delta.y = (MouseLocation.Y - m_pointOldPos.Y) * pct;

		if( m_bDraggingCamera )
		{
			m_camera.X += MouseLocation.X - m_pointOldPos.X;
			m_camera.Y += MouseLocation.Y - m_pointOldPos.Y;

			m_pointOldPos = MouseLocation;
			InvalidateRect( hWnd, NULL, FALSE );
		}
		else
		{
			if( m_bDraggingVerts )
			{
				// Origin...
				if( m_origin.IsSel() )
				{
					// Adjust temp positions
					m_origin.TempX += Delta.x;
					m_origin.TempY -= Delta.y;

					// Snap real positions to the grid.
					m_origin.X = m_origin.TempX;
					m_origin.Y = m_origin.TempY;
					m_origin = m_origin.GridSnap( FVector(GGridSize, GGridSize, GGridSize) );
				}

				// Handles...
				for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
				{
					if( m_shapes(shape).Handle.IsSel() )
					{
						// Adjust temp positions
						m_shapes(shape).Handle.TempX += Delta.x;
						m_shapes(shape).Handle.TempY -= Delta.y;

						// Snap real positions to the grid.
						m_shapes(shape).Handle.X = m_shapes(shape).Handle.TempX;
						m_shapes(shape).Handle.Y = m_shapes(shape).Handle.TempY;

						// Also move all of this shapes vertices.
						for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
							for( int vertex = 0 ; vertex < 2 ; vertex++ )
							{
								F2DSEVector* Vertex = &(m_shapes(shape).Segments(seg).Vertex[vertex]);

								// Adjust temp positions
								Vertex->TempX += Delta.x;
								Vertex->TempY -= Delta.y;

								// Snap real positions to the grid.
								Vertex->X = Vertex->TempX;
								Vertex->Y = Vertex->TempY;
								*Vertex = Vertex->GridSnap( FVector(GGridSize, GGridSize, GGridSize) );

								if( m_shapes(shape).Segments(seg).SegType == eSEGTYPE_BEZIER )
								{
									F2DSEVector* Vertex = &(m_shapes(shape).Segments(seg).ControlPoint[vertex]);

									// Adjust temp positions
									Vertex->TempX += Delta.x;
									Vertex->TempY -= Delta.y;

									// Snap real positions to the grid.
									Vertex->X = Vertex->TempX;
									Vertex->Y = Vertex->TempY;
									*Vertex = Vertex->GridSnap( FVector(GGridSize, GGridSize, GGridSize) );
								}
							}
					}
				}

				// Vertices...
				for( shape = 0 ; shape < m_shapes.Num() ; shape++ )
					if( !m_shapes(shape).Handle.IsSel() )
						for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
						{
							for( int vertex = 0 ; vertex < 2 ; vertex++ )
							{
								if( m_shapes(shape).Segments(seg).Vertex[vertex].IsSel() )
								{
									F2DSEVector* Vertex = &(m_shapes(shape).Segments(seg).Vertex[vertex]);

									// Adjust temp positions
									Vertex->TempX += Delta.x;
									Vertex->TempY -= Delta.y;

									// Snap real positions to the grid.
									Vertex->X = Vertex->TempX;
									Vertex->Y = Vertex->TempY;
									*Vertex = Vertex->GridSnap( FVector(GGridSize, GGridSize, GGridSize) );
								}
								if( m_shapes(shape).Segments(seg).SegType == eSEGTYPE_BEZIER
										&& m_shapes(shape).Segments(seg).ControlPoint[vertex].IsSel() )
								{
									F2DSEVector* Vertex = &(m_shapes(shape).Segments(seg).ControlPoint[vertex]);

									// Adjust temp positions
									Vertex->TempX += Delta.x;
									Vertex->TempY -= Delta.y;

									// Snap real positions to the grid.
									Vertex->X = Vertex->TempX;
									Vertex->Y = Vertex->TempY;
									*Vertex = Vertex->GridSnap( FVector(GGridSize, GGridSize, GGridSize) );
								}
							}
						}

			}
		}

		m_pointOldPos = MouseLocation;
		ComputeHandlePositions();
		InvalidateRect( hWnd, NULL, FALSE );

		InvalidateRect( hWnd, NULL, FALSE );

		WWindow::OnMouseMove( Flags, MouseLocation );
		unguard;
	}
	void ComputeHandlePositions( UBOOL bAlwaysCompute = FALSE)
	{
		guard(W2DShapeEditor::ComputeHandlePositions);
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			if( !m_shapes(shape).Handle.IsSel() || bAlwaysCompute )
				m_shapes(shape).ComputeHandlePosition();
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void ChangeSegmentTypes( int InType )
	{
		guard(W2DShapeEditor::ChangeSegmentTypes);

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				if( m_shapes(shape).Segments(seg).IsSel() )
					m_shapes(shape).Segments(seg).SetSegType(InType);

		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void SetSegmentDetail( int InDetailLevel )
	{
		guard(W2DShapeEditor::SetSegmentDetail);
		GDefaultDetailLevel = InDetailLevel;
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				if( m_shapes(shape).Segments(seg).IsSel() )
					m_shapes(shape).Segments(seg).DetailLevel = InDetailLevel;

		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void OnLeftButtonDown()
	{
		guard(W2DShapeEditor::OnLeftButtonDown);
		
		m_bDraggingVerts = TRUE;
		m_pointOldPos = GetCursorPos();

		if( GetAsyncKeyState(VK_CONTROL) & 0x8000 )
			ProcessHits( 0, TRUE );
		else 
		{
			// If the user has clicked on a vertex, then select that vertex and put them into drag mode.  Otherwise,
			// leave the current selections alone and just drag them.
			if( ProcessHits( 1, TRUE ) )
				ProcessHits( 0, FALSE );
			else
				DeselectAllVerts();
		}

		// Set the temp positions on all vertices.
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Handle.SetTempPos();
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				for( int vertex = 0 ; vertex < 2 ; vertex++ )
				{
					m_shapes(shape).Segments(seg).Vertex[vertex].SetTempPos();
					m_shapes(shape).Segments(seg).ControlPoint[vertex].SetTempPos();
				}
		}

		InvalidateRect( hWnd, NULL, FALSE );

		WWindow::OnLeftButtonDown();
		unguard;
	}
	void OnLeftButtonUp()
	{
		guard(W2DShapeEditor::OnLeftButtonUp);

		ReleaseCapture();
		m_bDraggingVerts = FALSE;

		if( !m_bMouseHasMoved && !(GetAsyncKeyState(VK_CONTROL) & 0x8000) )
			DeselectAllVerts();
		ComputeHandlePositions();

		WWindow::OnLeftButtonUp();
		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(W2DShapeEditor::OnSize);
		PositionChildControls();
		WWindow::OnSize(Flags, NewX, NewY);
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void PositionChildControls( void )
	{
		guard(W2DShapeEditor::PositionChildControls);

		if( !::IsWindow( GetDlgItem( hWnd, ID_2DSE_TOOLBAR )))	return;

		LockWindowUpdate( hWnd );

		FRect CR = GetClientRect();
		RECT R;
		::GetWindowRect( GetDlgItem( hWnd, ID_2DSE_TOOLBAR ), &R );
		::MoveWindow( GetDlgItem( hWnd, ID_2DSE_TOOLBAR ), 0, 0, CR.Max.X, R.bottom, TRUE );

		LockWindowUpdate( NULL );

		unguard;
	}
	virtual void OnKeyDown( TCHAR Ch )
	{
		guard(W2DShapeEditor::OnKeyDown);
		// Hot keys from old version
		if( Ch == 'I' && GetKeyState(VK_CONTROL) & 0x8000)
			SplitSides();
		if( Ch == VK_DELETE )
			Delete();
		unguard;
	}
	// Rotate the shapes by the speifued angle, around the origin,
	void RotateShape( int _Angle )
	{
		guard(W2DShapeEditor::RotateShape);
		FVector l_vec;
		FRotator StepRotation( 0, (65536.0f / 360.0f)  * _Angle, 0 );

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				for( int vertex = 0 ; vertex < 2 ; vertex++ )
				{
					l_vec.X = m_shapes(shape).Segments(seg).Vertex[vertex].X;	l_vec.Y = m_shapes(shape).Segments(seg).Vertex[vertex].Y;	l_vec.Z = m_shapes(shape).Segments(seg).Vertex[vertex].Z;
					l_vec = m_origin + ( l_vec - m_origin ).TransformVectorBy( GMath.UnitCoords * StepRotation);

					m_shapes(shape).Segments(seg).Vertex[vertex].X = l_vec.X;	m_shapes(shape).Segments(seg).Vertex[vertex].Y = l_vec.Y;	m_shapes(shape).Segments(seg).Vertex[vertex].Z = l_vec.Z;
					m_shapes(shape).Segments(seg).Vertex[vertex] = m_shapes(shape).Segments(seg).Vertex[vertex].GridSnap( FVector( GGridSize, GGridSize, GGridSize ) );

					m_shapes(shape).Segments(seg).ControlPoint[vertex].X = l_vec.X;	m_shapes(shape).Segments(seg).ControlPoint[vertex].Y = l_vec.Y;	m_shapes(shape).Segments(seg).ControlPoint[vertex].Z = l_vec.Z;
					m_shapes(shape).Segments(seg).ControlPoint[vertex] = m_shapes(shape).Segments(seg).ControlPoint[vertex].GridSnap( FVector( GGridSize, GGridSize, GGridSize ) );
				}

		ComputeHandlePositions(1);
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	// Flips the shape across the origin.
	void Flip( BOOL _bHoriz )
	{
		guard(W2DShapeEditor::Flip);

		// Flip the vertices across the origin.
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
			{
				for( int vertex = 0 ; vertex < 2 ; vertex++ )
				{
					FVector Dist = m_shapes(shape).Segments(seg).Vertex[vertex] - m_origin;
					FVector CPDist = m_shapes(shape).Segments(seg).ControlPoint[vertex] - m_origin;

					if( _bHoriz )
					{
						m_shapes(shape).Segments(seg).Vertex[vertex].X -= (Dist.X * 2);
						m_shapes(shape).Segments(seg).ControlPoint[vertex].X -= (CPDist.X * 2);
					}
					else
					{
						m_shapes(shape).Segments(seg).Vertex[vertex].Y -= (Dist.Y * 2);
						m_shapes(shape).Segments(seg).ControlPoint[vertex].Y -= (CPDist.Y * 2);
					}
				}
				Exchange( m_shapes(shape).Segments(seg).Vertex[0], m_shapes(shape).Segments(seg).Vertex[1] );
				Exchange( m_shapes(shape).Segments(seg).ControlPoint[0], m_shapes(shape).Segments(seg).ControlPoint[1] );
			}
		}


		ComputeHandlePositions(1);
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void DeselectAllVerts()
	{
		guard(W2DShapeEditor::DeselectAllVerts);
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Handle.Select(0);
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
			{
				m_shapes(shape).Segments(seg).Vertex[0].Select(0);
				m_shapes(shape).Segments(seg).Vertex[1].Select(0);
				m_shapes(shape).Segments(seg).ControlPoint[0].Select(0);
				m_shapes(shape).Segments(seg).ControlPoint[1].Select(0);
			}
		}
		m_origin.Select(0);
		unguard;
	}
	BOOL ProcessHits( BOOL bJustChecking, BOOL _bCumulative )
	{
		guard(W2DShapeEditor::ProcessHits);

		if( !_bCumulative )
			DeselectAllVerts();

		// Get the click position in world space.
		//
		FPoint l_click = GetCursorPos();

		l_click.X += -m_camera.X - (m_rcWnd.right / 2);
		l_click.Y += -m_camera.Y - (m_rcWnd.bottom / 2);

		//
		// See if any vertex comes within the selection radius to this point.  If so, select it...
		//

		// Check origin ...
		F2DSEVector l_vtxTest;
		l_vtxTest.X = (float)l_click.X - (m_origin.X * Zoom);
		l_vtxTest.Y = (float)l_click.Y + (m_origin.Y * Zoom);
		if( l_vtxTest.Size() <= d2dSE_SELECT_TOLERANCE )
		{
			if( !bJustChecking )
				m_origin.SelectToggle();
			return TRUE;
		}

		// Check shape handles...
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			l_vtxTest.X = (float)l_click.X - (m_shapes(shape).Handle.X * Zoom);
			l_vtxTest.Y = (float)l_click.Y + (m_shapes(shape).Handle.Y * Zoom);

			if( l_vtxTest.Size() <= d2dSE_SELECT_TOLERANCE )
			{
				if( !bJustChecking )
					m_shapes(shape).Handle.SelectToggle();
				return TRUE;
			}
		}

		// Check vertices...
		for( shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				for( int vertex = 0 ; vertex < 2 ; vertex++ )
				{
					l_vtxTest.X = (float)l_click.X - (m_shapes(shape).Segments(seg).Vertex[vertex].X * Zoom);
					l_vtxTest.Y = (float)l_click.Y + (m_shapes(shape).Segments(seg).Vertex[vertex].Y * Zoom);

					if( l_vtxTest.Size() <= d2dSE_SELECT_TOLERANCE )
						if( bJustChecking )
							return TRUE;
						else
							m_shapes(shape).Segments(seg).Vertex[vertex].SelectToggle();

					l_vtxTest.X = (float)l_click.X - (m_shapes(shape).Segments(seg).ControlPoint[vertex].X * Zoom);
					l_vtxTest.Y = (float)l_click.Y + (m_shapes(shape).Segments(seg).ControlPoint[vertex].Y * Zoom);

					if( l_vtxTest.Size() <= d2dSE_SELECT_TOLERANCE )
						if( bJustChecking )
							return TRUE;
						else
							m_shapes(shape).Segments(seg).ControlPoint[vertex].SelectToggle();
				}

		return FALSE;
		unguard;
	}
	void New( void )
	{
		guard(W2DShapeEditor::New);
		m_camera.X = m_camera.Y = 0;
		m_origin.X = 0; m_origin.Y = 0;

		m_shapes.Empty();
		InsertNewShape();

		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void InsertNewShape()
	{
		guard(W2DShapeEditor::InsertNewShape);

		F2DSEVector A(-128,-128,0), B(-128,128,0), C(128,128,0), D(128,-128,0);
		new(m_shapes)FShape();
		new(m_shapes(m_shapes.Num()-1).Segments)FSegment(A, B);
		new(m_shapes(m_shapes.Num()-1).Segments)FSegment(B, C);
		new(m_shapes(m_shapes.Num()-1).Segments)FSegment(C, D);
		new(m_shapes(m_shapes.Num()-1).Segments)FSegment(D, A);
		ComputeHandlePositions();

		unguard;
	}
	void DrawGrid( HDC _hdc )
	{
		guard(W2DShapeEditor::DrawGrid);
		int l_iXStart, l_iYStart, l_iXEnd, l_iYEnd;
		FVector l_vecTopLeft;
		HPEN l_penOriginLines, l_penMajorLines, l_penMinorLines, l_penOld;

		float GGridSizeZoomSave = GGridSizeZoom;
		GGridSizeZoom = Max( (GGridSizeZoom * (Zoom <= 1 ? Zoom : 1)), 8.0f);
		
		l_vecTopLeft.X = (m_camera.X * -1) - (m_rcWnd.right / 2);
		l_vecTopLeft.Y = (m_camera.Y * -1) - (m_rcWnd.bottom / 2);

		l_penMinorLines = CreatePen( PS_SOLID, 1, RGB( 235, 235, 235 ) );
		l_penMajorLines = CreatePen( PS_SOLID, 1, RGB( 215, 215, 215 ) );
		l_penOriginLines = CreatePen( PS_SOLID, 3, RGB( 225, 225, 225 ) );

		// Snap the starting position to the grid size.
		//
		l_iXStart = GGridSizeZoom - (int)l_vecTopLeft.X % GGridSizeZoom;
		l_iYStart = GGridSizeZoom - (int)l_vecTopLeft.Y % GGridSizeZoom;

		l_iXEnd = l_iXStart + m_rcWnd.right;
		l_iYEnd = l_iYStart + m_rcWnd.bottom;
		
		// Draw the lines.
		//
		l_penOld = (HPEN)SelectObject( _hdc, l_penMinorLines );

		for( int y = l_iYStart ; y < l_iYEnd ; y += GGridSizeZoom )
		{
			if( l_vecTopLeft.Y + y == 0 )
				SelectObject( _hdc, l_penOriginLines );
			else
				if( !((int)(l_vecTopLeft.Y + y) % (int)(128 * Zoom)) )
					SelectObject( _hdc, l_penMajorLines );
				else
					SelectObject( _hdc, l_penMinorLines );

			::MoveToEx( _hdc, 0, y, NULL );
			::LineTo( _hdc, m_rcWnd.right, y );
		}

		for( int x = l_iXStart ; x < l_iXEnd ; x += GGridSizeZoom )
		{
			if( l_vecTopLeft.X + x == 0 )
				SelectObject( _hdc, l_penOriginLines );
			else
				if( !((int)(l_vecTopLeft.X + x) % (int)(128 * Zoom)) )
					SelectObject( _hdc, l_penMajorLines );
				else
					SelectObject( _hdc, l_penMinorLines );

			::MoveToEx( _hdc, x, 0, NULL );
			::LineTo( _hdc, x, m_rcWnd.bottom );
		}

		GGridSizeZoom = GGridSizeZoomSave;

		SelectObject( _hdc, l_penOld );
		DeleteObject( l_penOriginLines );
		DeleteObject( l_penMinorLines );
		DeleteObject( l_penMajorLines );
		unguard;
	}
	void DrawImage( HDC _hdc )
	{
		guard(W2DShapeEditor::DrawImage);
		if( !hImage ) return;

		HDC hdcMem;
		HBITMAP hbmOld;
		BITMAP bitmap;
		FVector l_vecLoc;

		l_vecLoc.X = m_camera.X + (m_rcWnd.right / 2);
		l_vecLoc.Y = m_camera.Y + (m_rcWnd.bottom / 2);

		// Prepare the bitmap.
		//
		GetObjectA( hImage, sizeof(BITMAP), (LPSTR)&bitmap );
		hdcMem = CreateCompatibleDC(_hdc);
		hbmOld = (HBITMAP)SelectObject(hdcMem, hImage);

		// Display it.
		//
		StretchBlt(_hdc,
			   l_vecLoc.X - ((bitmap.bmWidth * Zoom) / 2), l_vecLoc.Y - ((bitmap.bmHeight * Zoom) / 2),
			   bitmap.bmWidth * Zoom, bitmap.bmHeight * Zoom,
			   hdcMem,
			   0, 0,
			   bitmap.bmWidth, bitmap.bmHeight,
			   SRCCOPY);

		// Clean up.
		//
		SelectObject(hdcMem, hbmOld);
		DeleteDC(hdcMem);
		unguard;
	}
	void DrawOrigin( HDC _hdc )
	{
		guard(W2DShapeEditor::DrawOrigin);
		HPEN l_pen, l_penSel, l_penOld;

		FVector l_vecLoc = m_camera;
		l_vecLoc.X += m_rcWnd.right / 2;
		l_vecLoc.Y += m_rcWnd.bottom / 2;

		l_pen = CreatePen( PS_SOLID, 2, RGB( 0, 255, 0 ) );
		l_penSel = CreatePen( PS_SOLID, 4, RGB( 255, 0, 0 ) );

		l_penOld = (HPEN)SelectObject( _hdc, (m_origin.IsSel() ? l_penSel : l_pen) );
		Rectangle( _hdc,
			l_vecLoc.X + (m_origin.X * Zoom) - 4,
			l_vecLoc.Y - (m_origin.Y * Zoom) + 4,
			l_vecLoc.X + (m_origin.X * Zoom) + 4,
			l_vecLoc.Y - (m_origin.Y * Zoom) - 4 );

		SelectObject( _hdc, l_penOld );
		DeleteObject( l_pen );
		DeleteObject( l_penSel );
		unguard;
	}
	void DrawSegments( HDC _hdc )
	{
		guard(W2DShapeEditor::DrawSegments);
		FVector l_vecLoc = m_camera;
		HPEN l_penLine, l_penBold, l_penCP, l_penCPBold, l_penOld;

		l_penLine = CreatePen( PS_SOLID, 1, RGB( 128, 128, 128 ) );
		l_penBold = CreatePen( PS_SOLID, 3, RGB( 128, 128, 128 ) );
		l_penCP = CreatePen( PS_SOLID, 1, RGB( 0, 0, 255 ) );
		l_penCPBold = CreatePen( PS_SOLID, 2, RGB( 0, 0, 255 ) );

		// Figure out where the top left corner of the window is in world coords.
		//
		l_vecLoc.X += m_rcWnd.right / 2;
		l_vecLoc.Y += m_rcWnd.bottom / 2;

		l_penOld = (HPEN)SelectObject( _hdc, l_penLine );

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
			{
				switch( m_shapes(shape).Segments(seg).SegType )
				{
					case eSEGTYPE_LINEAR:
						SelectObject( _hdc, (m_shapes(shape).Segments(seg).IsSel() ? l_penBold : l_penLine) );
						::MoveToEx( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).Vertex[0].X * Zoom),
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).Vertex[0].Y * Zoom), NULL );
						::LineTo( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).Vertex[1].X * Zoom),
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).Vertex[1].Y * Zoom) );
						break;

					case eSEGTYPE_BEZIER:

						// Generate list of vertices for bezier curve and render them as a line.
						TArray<FVector> BezierPoints;
						m_shapes(shape).Segments(seg).GetBezierPoints( &BezierPoints );
						SelectObject( _hdc, (m_shapes(shape).Segments(seg).IsSel() ? l_penBold : l_penLine) );

						for( int bz = 0 ; bz < BezierPoints.Num() - 1 ; bz++ )
						{
							::MoveToEx( _hdc,
								l_vecLoc.X + (BezierPoints(bz).X * Zoom),
								l_vecLoc.Y - (BezierPoints(bz).Y * Zoom), NULL );
							::LineTo( _hdc,
								l_vecLoc.X + (BezierPoints(bz+1) * Zoom).X,
								l_vecLoc.Y - (BezierPoints(bz+1) * Zoom).Y );
						}

						// Render the control points and connecting lines.
						SelectObject( _hdc, (m_shapes(shape).Segments(seg).ControlPoint[0].IsSel() ? l_penCPBold : l_penCP) );
						::MoveToEx( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).Vertex[0].X * Zoom),
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).Vertex[0].Y * Zoom), NULL );
						::LineTo( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).ControlPoint[0].X * Zoom),
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).ControlPoint[0].Y * Zoom) );

						SelectObject( _hdc, (m_shapes(shape).Segments(seg).ControlPoint[1].IsSel() ? l_penCPBold : l_penCP) );
						::MoveToEx( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).Vertex[1].X * Zoom),
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).Vertex[1].Y * Zoom), NULL );
						::LineTo( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).ControlPoint[1].X * Zoom),
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).ControlPoint[1].Y * Zoom) );
						break;
				}
			}
		}

		SelectObject( _hdc, l_penOld );
		DeleteObject( l_penLine );
		DeleteObject( l_penBold );
		DeleteObject( l_penCP );
		DeleteObject( l_penCPBold );
		unguard;
	}
	void DrawShapeHandles( HDC _hdc )
	{
		guard(W2DShapeEditor::DrawShapeHandles);
		FVector l_vecLoc = m_camera;
		HPEN l_penLine, l_penBold, l_penOld;

		l_penLine = CreatePen( PS_SOLID, 1, RGB( 255, 128, 0 ) );
		l_penBold = CreatePen( PS_SOLID, 3, RGB( 255, 128, 0 ) );

		// Figure out where the top left corner of the window is in world coords.
		//
		l_vecLoc.X += m_rcWnd.right / 2;
		l_vecLoc.Y += m_rcWnd.bottom / 2;

		l_penOld = (HPEN)SelectObject( _hdc, l_penLine );

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			SelectObject( _hdc, (m_shapes(shape).Handle.IsSel() ? l_penBold : l_penLine) );
			Rectangle( _hdc,
				l_vecLoc.X + (m_shapes(shape).Handle.X * Zoom) - 4,
				l_vecLoc.Y - (m_shapes(shape).Handle.Y * Zoom) + 4,
				l_vecLoc.X + (m_shapes(shape).Handle.X * Zoom) + 4,
				l_vecLoc.Y - (m_shapes(shape).Handle.Y * Zoom) - 4 );
		}

		SelectObject( _hdc, l_penOld );
		DeleteObject( l_penLine );
		DeleteObject( l_penBold );
		unguard;
	}
	void DrawVertices( HDC _hdc )
	{
		guard(W2DShapeEditor::DrawVertices);
		FVector l_vecLoc = m_camera;
		HPEN l_penVertex, l_penVertexBold, l_penCP, l_penCPBold, l_penOld;

		l_penVertex = CreatePen( PS_SOLID, 1, RGB( 0, 0, 0 ) );
		l_penVertexBold = CreatePen( PS_SOLID, 3, RGB( 255, 0, 0 ) );
		l_penCP = CreatePen( PS_SOLID, 1, RGB( 0, 0, 255 ) );
		l_penCPBold = CreatePen( PS_SOLID, 3, RGB( 0, 0, 255 ) );

		// Figure out where the top left corner of the window is in world coords.
		//
		l_vecLoc.X += m_rcWnd.right / 2;
		l_vecLoc.Y += m_rcWnd.bottom / 2;

		// Draw the vertices.
		//
		l_penOld = (HPEN)SelectObject( _hdc, l_penVertex );
		HBRUSH OldBrush = (HBRUSH)SelectObject( _hdc, GetStockObject(WHITE_BRUSH) );

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				for( int vertex = 0 ; vertex < 2 ; vertex++ )
				{
					SelectObject( _hdc, (m_shapes(shape).Segments(seg).Vertex[vertex].IsSel() ? l_penVertexBold : l_penVertex) );
					Rectangle( _hdc,
						l_vecLoc.X + (m_shapes(shape).Segments(seg).Vertex[vertex].X * Zoom) - 4,
						l_vecLoc.Y - (m_shapes(shape).Segments(seg).Vertex[vertex].Y * Zoom) + 4,
						l_vecLoc.X + (m_shapes(shape).Segments(seg).Vertex[vertex].X * Zoom) + 4,
						l_vecLoc.Y - (m_shapes(shape).Segments(seg).Vertex[vertex].Y * Zoom) - 4 );

					if( m_shapes(shape).Segments(seg).SegType == eSEGTYPE_BEZIER )
					{
						SelectObject( _hdc, (m_shapes(shape).Segments(seg).ControlPoint[vertex].IsSel() ? l_penCPBold : l_penCP) );
						Rectangle( _hdc,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).ControlPoint[vertex].X * Zoom) - 4,
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).ControlPoint[vertex].Y * Zoom) + 4,
							l_vecLoc.X + (m_shapes(shape).Segments(seg).ControlPoint[vertex].X * Zoom) + 4,
							l_vecLoc.Y - (m_shapes(shape).Segments(seg).ControlPoint[vertex].Y * Zoom) - 4 );
					}
				}

		SelectObject( _hdc, l_penOld );
		SelectObject( _hdc, OldBrush );
		DeleteObject( l_penVertex );
		DeleteObject( l_penVertexBold );
		DeleteObject( l_penCP );
		DeleteObject( l_penCPBold );
		unguard;
	}
	// Splits all selected sides in half.
	void SplitSides( void )
	{
		guard(W2DShapeEditor::SplitSides);

		// Break each selected segment into two.
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				if( m_shapes(shape).Segments(seg).IsSel() )
				{
					// Create a new segment half the size of this one, starting from the middle and extending
					// to the second vertex.
					FVector HalfWay = m_shapes(shape).Segments(seg).GetHalfwayPoint();
					new(m_shapes(shape).Segments)FSegment( HalfWay, m_shapes(shape).Segments(seg).Vertex[1] );

					// Move the original segments ending point to the halfway point.
					m_shapes(shape).Segments(seg).Vertex[1] = HalfWay;
				}

		ComputeHandlePositions();
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void Delete( void )
	{
		guard(W2DShapeEditor::Delete);

		// Delete any vertices which are selected on the current shape.
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			if( m_shapes(shape).Handle.IsSel() )
			{
				m_shapes.Remove(shape);
				shape = 0;
			}
			else
				if( m_shapes(shape).Segments.Num() > 3 )
					for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
						if( m_shapes(shape).Segments(seg).IsSel() )
						{
							FVector HalfWay = m_shapes(shape).Segments(seg).GetHalfwayPoint();
							F2DSEVector v1, v2;

							v1 = m_shapes(shape).Segments(seg).Vertex[0];
							v2 = m_shapes(shape).Segments(seg).Vertex[1];

							m_shapes(shape).Segments.Remove(seg);

							for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
							{
								if( m_shapes(shape).Segments(seg).Vertex[0] == v2 )
									m_shapes(shape).Segments(seg).Vertex[0] = HalfWay;
								if( m_shapes(shape).Segments(seg).Vertex[1] == v1 )
									m_shapes(shape).Segments(seg).Vertex[1] = HalfWay;
							}
						}
		}
				
		ComputeHandlePositions();
		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void DeleteShape( void )
	{
		guard(W2DShapeEditor::DeleteShape);

		// Deletes any shapes which have at least one vertex selected on them.

		InvalidateRect( hWnd, NULL, FALSE );
		unguard;
	}
	void FileSaveAs( HWND hWnd )
	{
		OPENFILENAMEA ofn;
		char File[8192] = "\0";
		strcpy( File, TCHAR_TO_ANSI( MapFilename ) );

		ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
		ofn.lStructSize = sizeof(OPENFILENAMEA);
		ofn.hwndOwner = hWnd;
		ofn.lpstrFile = File;
		ofn.nMaxFile = sizeof(char) * 8192;
		ofn.lpstrFilter = "2D Shapes (*.2ds)\0*.2ds\0All Files\0*.*\0\0";
		ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_2DS]) );
		ofn.lpstrDefExt = "2ds";
		ofn.lpstrTitle = "Save 2D Shape";
		ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

		// Display the Open dialog box. 
		//
		if( GetSaveFileNameA(&ofn) )
		{
			appStrcpy( MapFilename, appFromAnsi( File ) );
			WriteShape( MapFilename );

			FString S = MapFilename;
			GLastDir[eLASTDIR_2DS] = S.Left( S.InStr( TEXT("\\"), 1 ) );
		}

		SetCaption();
	}
	void FileSave( HWND hWnd )
	{
		if( ::appStrlen( MapFilename ) )
			WriteShape( MapFilename );
		else
			FileSaveAs( hWnd );
	}
	void FileOpen( HWND hWnd )
	{
		OPENFILENAMEA ofn;
		char File[8192] = "\0";

		ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
		ofn.lStructSize = sizeof(OPENFILENAMEA);
		ofn.hwndOwner = hWnd;
		ofn.lpstrFile = File;
		ofn.nMaxFile = sizeof(char) * 8192;
		ofn.lpstrFilter = "2D Shapes (*.2ds)\0*.2ds\0All Files\0*.*\0\0";
		ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_2DS]) );
		ofn.lpstrDefExt = "2ds";
		ofn.lpstrTitle = "Open 2D Shape";
		ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR;

		// Display the Open dialog box. 
		//
		if( GetOpenFileNameA(&ofn) )
		{
			appStrcpy( MapFilename, appFromAnsi( File ) );
			ReadShape( MapFilename );
			SetCaption();

			FString S = MapFilename;
			GLastDir[eLASTDIR_2DS] = S.Left( S.InStr( TEXT("\\"), 1 ) );
		}
	}
	void SetCaption( void )
	{
		TCHAR l_chCaption[256];

		appSprintf( l_chCaption, TEXT("2D Shape Editor - [%s]"), MapFilename );
		SetText( l_chCaption );
	}
	void WriteShape( TCHAR* Filename )
	{
		FArchive* Archive;
		Archive = GFileManager->CreateFileWriter( Filename );

		if( Archive )
		{
			// Origin
			//
			*Archive << m_origin.X << m_origin.Y;

			// Shapes
			//
			int Num = m_shapes.Num();
			*Archive << Num;
			for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			{
				Num = m_shapes(shape).Segments.Num();
				*Archive << Num;
				for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				{
					FSegment* pSeg = &(m_shapes(shape).Segments(seg));
					*Archive << pSeg->Vertex[0].X << pSeg->Vertex[0].Y << pSeg->Vertex[0].Z
						<< pSeg->Vertex[1].X << pSeg->Vertex[1].Y << pSeg->Vertex[1].Z
						<< pSeg->ControlPoint[0].X << pSeg->ControlPoint[0].Y << pSeg->ControlPoint[0].Z
						<< pSeg->ControlPoint[1].X << pSeg->ControlPoint[1].Y << pSeg->ControlPoint[1].Z
						<< pSeg->SegType << pSeg->DetailLevel;
				}
			}

			Archive->Close();
		}
	}
	void ReadShape( TCHAR* Filename )
	{
		FArchive* Archive;
		Archive = GFileManager->CreateFileReader( Filename );

		if( Archive )
		{
			m_camera.X = m_camera.Y = 0;
			m_shapes.Empty();

			// Origin
			//
			Archive->Serialize( &m_origin.X, sizeof(float) );
			Archive->Serialize( &m_origin.Y, sizeof(float) );
			

			// Shapes
			//
			int NumShapes;
			Archive->Serialize( &NumShapes, sizeof(int) );

			for( int shape = 0 ; shape < NumShapes ; shape++ )
			{
				new(m_shapes)FShape();

				int NumSegments;
				Archive->Serialize( &NumSegments, sizeof(int) );

				for( int seg = 0 ; seg < NumSegments ; seg++ )
				{
					new(m_shapes(shape).Segments)FSegment;
					FSegment* pSeg = &(m_shapes(shape).Segments(seg));
					Archive->Serialize( &(pSeg->Vertex[0].X), sizeof(float) );
					Archive->Serialize( &(pSeg->Vertex[0].Y), sizeof(float) );
					Archive->Serialize( &(pSeg->Vertex[0].Z), sizeof(float) );
					Archive->Serialize( &(pSeg->Vertex[1].X), sizeof(float) );
					Archive->Serialize( &(pSeg->Vertex[1].Y), sizeof(float) );
					Archive->Serialize( &(pSeg->Vertex[1].Z), sizeof(float) );
					Archive->Serialize( &(pSeg->ControlPoint[0].X), sizeof(float) );
					Archive->Serialize( &(pSeg->ControlPoint[0].Y), sizeof(float) );
					Archive->Serialize( &(pSeg->ControlPoint[0].Z), sizeof(float) );
					Archive->Serialize( &(pSeg->ControlPoint[1].X), sizeof(float) );
					Archive->Serialize( &(pSeg->ControlPoint[1].Y), sizeof(float) );
					Archive->Serialize( &(pSeg->ControlPoint[1].Z), sizeof(float) );
					Archive->Serialize( &(pSeg->SegType), sizeof(float) );
					Archive->Serialize( &(pSeg->DetailLevel), sizeof(float) );
				}
				ComputeHandlePositions();
			}

			Archive->Close();
		}

		InvalidateRect( hWnd, NULL, FALSE );
	}
	void SetOrigin( void )
	{
		POINT l_click = m_ContextPos;

		l_click.x += -m_camera.X - (m_rcWnd.right / 2);
		l_click.y += -m_camera.Y - (m_rcWnd.bottom / 2);

		l_click.x -= l_click.x % GGridSize;
		l_click.y -= l_click.y % GGridSize;

		m_origin.X = l_click.x;
		m_origin.Y = -l_click.y;

		InvalidateRect( hWnd, NULL, FALSE );
	}
	void ProcessSheet()
	{
		guard(W2DShapeEditor::ProcessSheet)

		FString Cmd;

		// Reverse the Y Axis
		Flip(0);

		Cmd += TEXT("BRUSH SET\n\n");

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Tesselate(m_origin);
			for( TMap<int, CPOLY_TRIANGLE>::TIterator It(m_shapes(shape).Triangles); It ; ++It )
			{
				CPOLY_TRIANGLE* ptri = &(It.Value());
				Exchange( ptri->Vertex1, ptri->Vertex2 );

				Cmd += TEXT("Begin Polygon Flags=8\n");

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex1).XY.X, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex2).XY.X, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex3).XY.X, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));

				Cmd += TEXT("End Polygon\n");
			}
		}

		Flip(0);

		GEditor->Exec( *Cmd );
		unguard;
	}
	void ProcessExtrude( int Depth )
	{
		guard(W2DShapeEditor::ProcessExtrude)

		// Reverse the Y Axis
		Flip(0);

		FString Cmd;

		Cmd += TEXT("BRUSH SET\n\n");

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Tesselate(m_origin);

			// Top and bottom caps ...
			//
			for( TMap<int, CPOLY_TRIANGLE>::TIterator It(m_shapes(shape).Triangles); It ; ++It )
			{
				CPOLY_TRIANGLE* ptri = &(It.Value());

				Cmd += TEXT("Begin Polygon Flags=0\n");

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex1).XY.X, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y, Depth / 2.0f));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex2).XY.X, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y, Depth / 2.0f));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex3).XY.X, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y, Depth / 2.0f));

				Cmd += TEXT("End Polygon\n");

				Exchange( ptri->Vertex1, ptri->Vertex2 );

				Cmd += TEXT("Begin Polygon Flags=0\n");

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex1).XY.X, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y, -(Depth / 2.0f) ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex2).XY.X, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y, -(Depth / 2.0f) ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex3).XY.X, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y, -(Depth / 2.0f) ));

				Cmd += TEXT("End Polygon\n");
			}

			// Sides ...
			//
			for( int vtx = 0 ; vtx < m_shapes(shape).Vertices.Num() ; vtx++ )
			{
				Cmd += TEXT("Begin Polygon Flags=0\n");

				FVector* pvtxPrev = &( m_shapes(shape).Vertices( (vtx ? vtx - 1 : m_shapes(shape).Vertices.Num() - 1 ) ).XY );
				FVector* pvtx = &( m_shapes(shape).Vertices(vtx).XY );

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtx->X, pvtx->Y, Depth / 2.0f ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtxPrev->X, pvtxPrev->Y, Depth / 2.0f ));

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtxPrev->X, pvtxPrev->Y, -(Depth / 2.0f) ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtx->X, pvtx->Y, -(Depth / 2.0f) ));

				Cmd += TEXT("End Polygon\n");
			}
		}

		GEditor->Exec( *Cmd );

		Flip(0);

		unguard;
	}
	void ProcessExtrudeToPoint( int Depth )
	{
		guard(W2DShapeEditor::ProcessExtrudeToPoint)

		// Flip the Y Axis
		Flip(0);

		FString Cmd;

		Cmd += TEXT("BRUSH SET\n\n");

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Tesselate(m_origin);
			for( TMap<int, CPOLY_TRIANGLE>::TIterator It(m_shapes(shape).Triangles); It ; ++It )
			{
				CPOLY_TRIANGLE* ptri = &(It.Value());
				Exchange( ptri->Vertex1, ptri->Vertex2 );

				Cmd += TEXT("Begin Polygon Flags=0\n");

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex1).XY.X, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex2).XY.X, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex3).XY.X, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));

				Cmd += TEXT("End Polygon\n");
			}

			// Sides ...
			//
			for( int vtx = 0 ; vtx < m_shapes(shape).Vertices.Num() ; vtx++ )
			{
				Cmd += TEXT("Begin Polygon Flags=0\n");

				FVector* pvtxPrev = &( m_shapes(shape).Vertices( (vtx ? vtx - 1 : m_shapes(shape).Vertices.Num() - 1 ) ).XY );
				FVector* pvtx = &( m_shapes(shape).Vertices(vtx).XY );

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtxPrev->X, pvtxPrev->Y, pvtxPrev->Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtx->X, pvtx->Y, pvtx->Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_origin.X, m_origin.Y, m_origin.Z + Depth ));

				Cmd += TEXT("End Polygon\n");
			}
		}

		GEditor->Exec( *Cmd );

		Flip(0);

		unguard;
	}
	void ProcessExtrudeToBevel( int Depth, int CapHeight )
	{
		guard(W2DShapeEditor::ProcessExtrudeToBevel)

		// Flip the Y Axis
		Flip(0);

		FString Cmd;
		float Dist = 1.0f - (CapHeight / (float)Depth);

		Cmd += TEXT("BRUSH SET\n\n");

		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Tesselate(m_origin);
			for( TMap<int, CPOLY_TRIANGLE>::TIterator It(m_shapes(shape).Triangles); It ; ++It )
			{
				CPOLY_TRIANGLE* ptri = &(It.Value());
				Exchange( ptri->Vertex1, ptri->Vertex2 );

				Cmd += TEXT("Begin Polygon Flags=0\n");

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex1).XY.X, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex2).XY.X, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex3).XY.X, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));

				Cmd += TEXT("End Polygon\n");

				Exchange( ptri->Vertex1, ptri->Vertex2 );

				Cmd += TEXT("Begin Polygon Flags=0\n");

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex1).XY.X * Dist, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y * Dist, (float)CapHeight ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex2).XY.X * Dist, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y * Dist, (float)CapHeight ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					m_shapes(shape).Vertices(ptri->Vertex3).XY.X * Dist, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y * Dist, (float)CapHeight ));

				Cmd += TEXT("End Polygon\n");
			}

			// Sides ...
			//
			for( int vtx = 0 ; vtx < m_shapes(shape).Vertices.Num() ; vtx++ )
			{
				Cmd += TEXT("Begin Polygon Flags=0\n");

				FVector* pvtxPrev = &( m_shapes(shape).Vertices( (vtx ? vtx - 1 : m_shapes(shape).Vertices.Num() - 1 ) ).XY );
				FVector* pvtx = &( m_shapes(shape).Vertices(vtx).XY );

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtxPrev->X, pvtxPrev->Y, pvtxPrev->Z ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtx->X, pvtx->Y, pvtx->Z ));

				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtx->X * Dist, pvtx->Y * Dist, (float)CapHeight ));
				Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
					pvtxPrev->X * Dist, pvtxPrev->Y * Dist, (float)CapHeight ));

				Cmd += TEXT("End Polygon\n");
			}
		}

		GEditor->Exec( *Cmd );

		Flip(0);

		unguard;
	}
	void ProcessRevolve( int TotalSides, int Sides )
	{
		guard(W2DShapeEditor::ProcessRevolve)

		BOOL bPositive, bNegative;
		BOOL bFromLeftSide;

		// Make sure the origin is totally to the left or right of the shape.
		bPositive = bNegative = FALSE;
		for( int shape = 0 ; shape < m_shapes.Num() ; shape++ )
			for( int seg = 0 ; seg < m_shapes(shape).Segments.Num() ; seg++ )
				for( int vtx = 0 ; vtx < 2 ; vtx++ )
				{
					if( m_origin.X > m_shapes(shape).Segments(seg).Vertex[vtx].X )		bPositive = TRUE;
					if( m_origin.X < m_shapes(shape).Segments(seg).Vertex[vtx].X )		bNegative = TRUE;
				}

		if( bPositive && bNegative )
		{
			appMsgf( TEXT("Origin must be completely to the left or right side of the shape.") );
			return;
		}

		// When revolving from the left side, we have to flip the polys around.
		bFromLeftSide = ( bNegative && !bPositive );

		// Flip the Y Axis
		Flip(0);

		FString Cmd;

		Cmd += TEXT("BRUSH SET\n\n");

		for( shape = 0 ; shape < m_shapes.Num() ; shape++ )
		{
			m_shapes(shape).Tesselate(m_origin);

			if( Sides != TotalSides )	// Don't make end caps for a complete revolve
			{
				for( TMap<int, CPOLY_TRIANGLE>::TIterator It(m_shapes(shape).Triangles); It ; ++It )
				{
					CPOLY_TRIANGLE* ptri = &(It.Value());

					if( bFromLeftSide ) Exchange( ptri->Vertex1, ptri->Vertex2 );

					Cmd += TEXT("Begin Polygon Flags=0\n");

					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						m_shapes(shape).Vertices(ptri->Vertex1).XY.X, m_shapes(shape).Vertices(ptri->Vertex1).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						m_shapes(shape).Vertices(ptri->Vertex2).XY.X, m_shapes(shape).Vertices(ptri->Vertex2).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));
					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						m_shapes(shape).Vertices(ptri->Vertex3).XY.X, m_shapes(shape).Vertices(ptri->Vertex3).XY.Y, m_shapes(shape).Vertices(ptri->Vertex1).XY.Z ));

					Cmd += TEXT("End Polygon\n");

					// Flip/Rotate the triangle to create the end cap
					Exchange( ptri->Vertex1, ptri->Vertex2 );

					FRotator StepRotation( (65536.0f / TotalSides) * Sides, 0, 0 );
					FVector vtx1 = m_shapes(shape).Vertices(ptri->Vertex1).XY.TransformVectorBy( GMath.UnitCoords * StepRotation );
					FVector vtx2 = m_shapes(shape).Vertices(ptri->Vertex2).XY.TransformVectorBy( GMath.UnitCoords * StepRotation );
					FVector vtx3 = m_shapes(shape).Vertices(ptri->Vertex3).XY.TransformVectorBy( GMath.UnitCoords * StepRotation );

					Cmd += TEXT("Begin Polygon Flags=0\n");

					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtx1.X, vtx1.Y, vtx1.Z ));
					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtx2.X, vtx2.Y, vtx2.Z ));
					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtx3.X, vtx3.Y, vtx3.Z ));

					Cmd += TEXT("End Polygon\n");
				}
			}

			// Sides ...
			//
			for( int side = 0 ; side < Sides ; side++ )
				for( int vtx = 0 ; vtx < m_shapes(shape).Vertices.Num() ; vtx++ )
				{
					Cmd += TEXT("Begin Polygon Flags=0\n");

					FVector *pvtx, *pvtxPrev;

					pvtxPrev = &( m_shapes(shape).Vertices( (vtx ? vtx - 1 : m_shapes(shape).Vertices.Num() - 1 ) ).XY );
					pvtx = &( m_shapes(shape).Vertices(vtx).XY );

					if( bFromLeftSide )	Exchange( pvtxPrev, pvtx );

					FRotator StepRotation( (65536.0f / TotalSides) * side, 0, 0 ), StepRotation2( (65536.0f / TotalSides) * (side+1), 0, 0 );

					FVector vtxPrev = pvtxPrev->TransformVectorBy( GMath.UnitCoords * StepRotation );
					FVector vtx = pvtx->TransformVectorBy( GMath.UnitCoords * StepRotation );
					FVector vtxPrev2 = pvtxPrev->TransformVectorBy( GMath.UnitCoords * StepRotation2 );
					FVector vtx2 = pvtx->TransformVectorBy( GMath.UnitCoords * StepRotation2 );

					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtx.X, vtx.Y, vtx.Z ));
					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtxPrev.X, vtxPrev.Y, vtxPrev.Z ));

					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtxPrev2.X, vtxPrev2.Y, vtxPrev2.Z ));
					Cmd += *(FString::Printf(TEXT("Vertex   %1.1f, %1.1f, %1.1f\n"),
						vtx2.X, vtx2.Y, vtx2.Z ));

					Cmd += TEXT("End Polygon\n");
				}
		}

		GEditor->Exec( *Cmd );

		Flip(0);

		unguard;
	}
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
