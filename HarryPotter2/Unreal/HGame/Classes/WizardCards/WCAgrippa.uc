//===============================================================================
//  #2 Cornelius Agrippa 
//===============================================================================

class  WCAgrippa extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardAgrippaTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Agrippasmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardAgrippaBigTexture FILE=TEXTURES\menu\Folio\Cards\Agrippabig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Cornelius Agrippa";
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=2
	bVendorsCanSell=true
	strVendorOwnedAfterGState="GSTATE090"
    skin(0)=Texture'HProps.skins.WizardCardAgrippaTex0'
	textureBig=Texture'WizCardAgrippaBigTexture'
	strDescriptionId="WizCard_0040"
}
