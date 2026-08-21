//===============================================================================
// #16 Cliodne
//===============================================================================

class  WCCliodne extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardCliodneTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Cliodnesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardCliodneBigTexture FILE=TEXTURES\menu\Folio\Cards\Cliodnebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Cliodne";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=16
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE150"
    skin(0)=Texture'HProps.skins.WizardCardCliodneTex0'
	textureBig=Texture'WizCardCliodneBigTexture'
	strDescriptionId="WizCard_0054"
}
