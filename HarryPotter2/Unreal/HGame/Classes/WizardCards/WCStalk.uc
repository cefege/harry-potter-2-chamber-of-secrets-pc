//===============================================================================
// #85	Blenheim Stalk
//===============================================================================

class  WCStalk extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardStalkTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Stalksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardStalkBigTexture FILE=TEXTURES\menu\Folio\Cards\Stalkbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Blenheim Stalk";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=85
    skin(0)=Texture'HProps.skins.WizardCardStalkTex0'
	textureBig=Texture'WizCardStalkBigTexture'
	strDescriptionId="WizCard_0010"
}
