//===============================================================================
// #86 Dorcas Wellbeloved
//===============================================================================

class  WCWellbeloved extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWellbelovedTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wellbelovedsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWellbelovedBigTexture FILE=TEXTURES\menu\Folio\Cards\WellBelovedbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Dorcas Wellbeloved";

	Super.PostBeginPlay();
}

defaultproperties
{
	ID=86
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardWellbelovedTex0'
	textureBig=Texture'WizCardWellBelovedBigTexture'
	strDescriptionId="WizCard_0000"
}
