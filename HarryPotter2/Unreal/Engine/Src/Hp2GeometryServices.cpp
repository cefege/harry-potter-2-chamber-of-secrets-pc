/*=============================================================================
	Hp2GeometryServices.cpp: Warning sink for the Rust FPoly/UModel geometry port
	(crates/hp2-ffi/src/geometry.rs). Rust formats each message; C++ emits it
	through debugf(NAME_Warning) exactly as the original FPoly code did.
=============================================================================*/

#include "EnginePrivate.h"
#include "Hp2GeometryServices.h"

static void GeometryWarn( const char* Msg )
{
	debugf( NAME_Warning, TEXT("%s"), ANSI_TO_TCHAR(Msg) );
}

static void GeometrySurfModifyItem( void* Model, INT Index )
{
	((UModel*)Model)->Surfs.ModifyItem( Index );
}

static void GeometryBrushPolyModifyItem( void* Actor, INT Index )
{
	((ABrush*)Actor)->Brush->Polys->Element.ModifyItem( Index );
}

static void GeometrySurfAddNode( void* Model, INT Surf, INT Node )
{
	((UModel*)Model)->Surfs(Surf).Nodes.AddItem( Node );
}

const Hp2GeometryServices GHp2GeometryServices =
{
	GeometryWarn,
	GeometrySurfModifyItem,
	GeometryBrushPolyModifyItem,
	GeometrySurfAddNode,
};
