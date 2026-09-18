//===============================================================================
// #38	Chauncey Oldridge
//===============================================================================

class  WCOldridge extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardOldridgeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Oldridgesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardOldridgeBigTexture FILE=TEXTURES\menu\Folio\Cards\Oldridgebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Chauncey Oldridge";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=38
    skin(0)=Texture'HProps.skins.WizardCardOldridgeTex0'
	textureBig=Texture'WizCardOldridgeBigTexture'
	strDescriptionId="WizCard_0034"
}
