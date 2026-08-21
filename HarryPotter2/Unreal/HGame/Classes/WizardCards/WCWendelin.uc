//===============================================================================
//  #20 Wendelin the Weird
//===============================================================================

class  WCWendelin extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardWendelinTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wendelinsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWendelinBigTexture FILE=TEXTURES\menu\Folio\Cards\Wendelinbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Wendelin the Weird";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=20
    skin(0)=Texture'HProps.skins.WizardCardWendelinTex0'
	textureBig=Texture'WizCardWendelinBigTexture'
	strDescriptionId="WizCard_0025"
}
