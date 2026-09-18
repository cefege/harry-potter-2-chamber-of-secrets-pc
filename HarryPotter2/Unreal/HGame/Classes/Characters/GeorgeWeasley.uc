class GeorgeWeasley extends Characters;

defaultproperties
{
	BumpLineSetPrefix="Grg";

    Mesh=SkeletalMesh'HPModels.skGeorgeWeasleyMesh'
    AmbientGlow=65
    CollisionRadius=15
    CollisionHeight=45
	VendorDialogSet=VDialog_GeorgeWeasley
	bLuringEnabled=true     // George and Fred are always together- only Fred
	                        // lures (it's too much if the both lure him
							// when they are standing together.
	GroundRunSpeed=220
}
