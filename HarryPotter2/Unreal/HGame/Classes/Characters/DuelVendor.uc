//===============================================================================
// 
//===============================================================================

class DuelVendor extends Characters;

defaultproperties	   
{
	DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skhp2_genmale1Mesh'

	CollisionHeight=42
	CollisionRadius=15
	IdleAnimName="idle"
	AmbientGlow=75

	bCollideActors=false
	bBlockActors=false
	bBlockPlayers=false

    bHidden=true

	CharacterSells=Sells_Duel
	VendorDialogSet=VDialog_DuelVendor
}

