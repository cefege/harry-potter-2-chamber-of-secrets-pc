//===============================================================================
// #76 Myron Wagtail
//===============================================================================

class  WCWagtail extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWagtailTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wagtailsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWagtailBigTexture FILE=TEXTURES\menu\Folio\Cards\Wagtailbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Myron Wagtail";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=76
    skin(0)=Texture'HProps.skins.WizardCardWagtailTex0'
	textureBig=Texture'WizCardWagtailBigTexture'
	strDescriptionId="WizCard_0070"
}
