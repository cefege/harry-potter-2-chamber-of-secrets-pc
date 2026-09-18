//===============================================================================
//  #83 Roderic Plumpton 
//===============================================================================

class  WCPlumpton extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardPlumptonTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Plumptonsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardPlumptonBigTexture FILE=TEXTURES\menu\Folio\Cards\Plumptonbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Roderic Plumpton";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=83
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE110"
    skin(0)=Texture'HProps.skins.WizardCardPlumptonTex0'
	textureBig=Texture'WizCardPlumptonBigTexture'
	strDescriptionId="WizCard_0077"
}
